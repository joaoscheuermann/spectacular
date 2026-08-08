import * as revisionPrompt from '../../prompts/revision.js';
import {
  GraphHistorySchema,
  GraphSchema,
  PlannedGraphSchema,
} from '../../schemas/graph.js';
import { completeStructured } from '../../structured.js';
import { evaluate } from '../../evaluation.js';
import type { WorkflowHandler } from '../../types/workflow.js';
import {
  applyLocalizedRevision,
  localizedRevisionCount,
  retiredNodeIds,
  revisionNodes,
} from './localized.js';

/**
 * Implements localized structural repair from MOSAIC 0.2 section 4.10 and
 * Appendix A.2, consuming one node-owned request and its complete evidence set.
 */
export const revision: WorkflowHandler = async (
  state,
  { input, options, runtime, hooks },
  { transition, fail },
) => {
  try {
    GraphHistorySchema.parse(state.graphs);

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
          node.status === 'needs_revision' &&
          (node.outcome === null || node.outcome.revisionRequest === null),
      )
    ) {
      return fail(new Error('Revision node is missing its runtime request.'));
    }
    if (targets.length === 0) {
      return fail(new Error('Localized revision has no pending request.'));
    }

    // Validate all queued requests before selecting one, preventing latent bad state.
    for (const candidate of targets) {
      const request = candidate.outcome?.revisionRequest;
      if (request === null || request === undefined) {
        return fail(new Error('Revision node is missing its runtime request.'));
      }
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
      target.termination = {
        type: 'revision_limit',
        status: 'blocked',
        limit: options.revision.max,
      };
      await runtime?.emit({
        type: 'node.status',
        stage: 'revision',
        revision: active.revision,
        nodeId: target.id,
        status: target.status,
      });
      return transition('schedule', state);
    }

    // IDs removed by earlier revisions cannot be recycled for different semantics.
    const retiredIds = retiredNodeIds(state.graphs);

    options.logger.info(
      {
        phase: 'localized',
        revision: active.revision + 1,
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
    const plan = PlannedGraphSchema.parse(
      await evaluate(
        hooks?.localizedRevision,
        { request: input, graph: active, target, retiredIds },
        ({ request, graph, target, retiredIds }) =>
          completeStructured({
            provider: options.provider,
            profile: options.models.revision,
            system: revisionPrompt.localizedSystem(),
            input: revisionPrompt.localizedUser(
              request,
              graph,
              target,
              retiredIds,
            ),
            schema: PlannedGraphSchema,
            runtime,
            stage: 'revision',
            nodeId: target.id,
            revision: active.revision,
          }),
      ),
    );

    // Preserve protected nodes and reset only the revisable, unstarted region.
    const revised = applyLocalizedRevision(
      active,
      plan,
      target,
      new Set(retiredIds),
    );

    await runtime?.emit({
      type: 'graph.revised',
      stage: 'revision',
      revision: revised.revision,
      nodeId: target.id,
      nodeIds: revised.nodes.map(({ id }) => id),
      ...(runtime.capture === 'io' ? { graph: revised } : {}),
    });

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
