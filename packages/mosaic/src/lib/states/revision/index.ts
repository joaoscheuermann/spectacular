import type { StateMachineHandler } from 'state-machine';

import * as revisionPrompt from '../../prompts/revision.js';
import { GraphSchema, PlannedGraphSchema } from '../../schemas/graph.js';
import { RevisionRequestSchema } from '../../schemas/revision.js';
import type { WorkflowContext, WorkflowState } from '../../types/workflow.js';
import {
  applyLocalizedRevision,
  localizedRevisionCount,
  retiredNodeIds,
  revisionNodes,
} from './localized.js';

/**
 * Implements localized structural repair from MOSAIC 0.2 section 4.9 and
 * Appendix A.2, consuming one node-owned request and its complete evidence set.
 */
export const revision: StateMachineHandler<
  WorkflowContext,
  WorkflowState
> = async (state, { input, options }, { transition, fail }) => {
  try {
    // Revisions operate on the latest graph snapshot without mutating its history.
    const active = state.graphs.at(-1);
    if (active === undefined) {
      return fail(new Error('Localized revision is missing an active graph.'));
    }

    // Revalidate the runtime-mutated graph before using it as planner evidence.
    GraphSchema.parse(active);

    // Wave order determines which outstanding revision request is consumed first.
    const targets = revisionNodes(active);

    // Every needs_revision decision must carry its runtime-materialized request.
    if (
      active.nodes.some(
        (node) =>
          node.status === 'needs_revision' && node.revisionRequest === null,
      )
    ) {
      return fail(new Error('Revision node is missing its runtime request.'));
    }
    if (targets.length === 0) {
      return fail(new Error('Localized revision has no pending request.'));
    }

    // Validate all queued requests before selecting one, preventing latent bad state.
    for (const candidate of targets) {
      const request = RevisionRequestSchema.parse(candidate.revisionRequest);
      if (request.goalId !== candidate.id) {
        return fail(
          new Error(
            `Localized revision request must target node ${candidate.id}.`,
          ),
        );
      }
    }

    // Only the first deterministic target is revised during this state transition.
    const target = targets[0];
    if (target === undefined) {
      return fail(new Error('Localized revision has no pending request.'));
    }

    // Count only snapshots appended after P0 and P1 as successful local revisions.
    const revisionCount = localizedRevisionCount(state.graphs);
    if (revisionCount >= options.revision.max) {
      // Limit exhaustion blocks the target without spending another provider call.
      target.status = 'blocked';
      return fail(new Error(`Node ${target.id} exceeded revision limit.`));
    }

    // IDs removed by earlier revisions cannot be recycled for different semantics.
    const retiredIds = retiredNodeIds(state.graphs);

    options.logger.info(
      {
        phase: 'localized',
        revision: state.graphs.length,
        nodeCount: active.nodes.length,
        targetId: target.id,
      },
      'generating graph',
    );

    /**
     * The localized prompt contains the request, active plan, semantic request,
     * and every ordered Observation without provider-opaque call IDs. Catalog
     * hints are intentionally absent from runtime revision.
     */
    const { structured } = await options.provider.complete({
      messages: [
        { role: 'system', content: revisionPrompt.localizedSystem() },
        {
          role: 'user',
          content: revisionPrompt.localizedUser(
            input,
            active,
            target,
            retiredIds,
          ),
        },
      ],
      model: options.models.default,
      schema: PlannedGraphSchema,
    });

    // Parse model planning fields before enforcing localized runtime invariants.
    const plan = PlannedGraphSchema.parse(structured);

    // Preserve protected nodes and reset only the revisable, unstarted region.
    const revised = applyLocalizedRevision(
      active,
      plan,
      target,
      new Set(retiredIds),
    );

    // Append the successful snapshot, then let scheduling recompute readiness.
    return transition('schedule', {
      ...state,
      graphs: [...state.graphs, revised],
    });
  } catch (error) {
    // Invalid evidence, provider output, or graph changes fail without appending.
    return fail(error);
  }
};
