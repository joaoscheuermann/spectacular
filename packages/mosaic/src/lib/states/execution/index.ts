import { AgentErrorObject, createAgent } from 'agent';
import { createMessageStorage } from 'messages';
import { createToolStorage, type Tool } from 'tool';

import * as executionPrompt from '../../prompts/execution.js';
import {
  createNodeDecisionSchema,
  createNodeOutcomeSchema,
} from '../../schemas/outcome.js';
import { resolveSkills } from '../bundle/menus.js';
import type { Graph, Node } from '../../types/graph.js';
import type { WorkflowContext, WorkflowHandler } from '../../types/workflow.js';
import { materializeObservations } from './observations.js';

/**
 * Implements the node executor and lifecycle from the MOSAIC paper, sections
 * 4.9-4.10 and Algorithm 1. In particular, section 4.10
 * says: "O scheduler o coloca em running." Doric groups ready nodes into a
 * concurrent wave; that batching policy is a local MOSAIC 0.2 runtime choice.
 */
export const execution: WorkflowHandler = async (
  state,
  { input, options },
  { transition, fail },
) => {
  try {
    const { graphs } = state;
    /** Mosaic stores graphs as a LIFO stack, so the last graph is active. */
    const graph = graphs.at(-1);

    if (graph === undefined) {
      return fail(new Error('Impossible to continue, missing active graph!'));
    }

    /** Scheduling marks the nodes that form this execution wave as ready. */
    const nodes = graph.nodes
      .filter(({ status }) => status === 'ready')
      .sort((left, right) => right.index - left.index);

    if (nodes.length === 0)
      return fail(new Error('Impossible to continue, missing ready nodes!'));

    /**
     * Execute the complete wave concurrently and wait for every node. Using
     * allSettled prevents one rejection from hiding later node state changes.
     * Algorithm 1 selects the next ready objective abstractly; it does not
     * prescribe sequential or concurrent dispatch.
     */
    const results = await Promise.allSettled(
      nodes.map((node) => execute(input, node, graph, options)),
    );

    const failure = results.find(
      (result): result is PromiseRejectedResult => result.status === 'rejected',
    );

    if (failure !== undefined) return fail(failure.reason);

    /**
     * Return to scheduling so downstream dependencies can become ready. This
     * realizes section 4.10: pending becomes ready after dependencies complete.
     */
    return transition('schedule', state);
  } catch (e) {
    return fail(e);
  }
};

const execute = async (
  input: string,
  node: Node,
  graph: Graph,
  options: WorkflowContext['options'],
): Promise<void> => {
  /** The runtime owns node status after scheduling hands the node to execution. */
  node.status = 'running';
  /** Keep history available when a bounded run exhausts after executing tools. */
  const messages = createMessageStorage();

  try {
    /** Resolve graph-approved tool metadata to executable catalog entries. */
    const tools = executors(node, options.tools.required, options.tools.menu);
    if (node.bundle === null) {
      throw new Error(`Node ${node.id} has not been routed.`);
    }
    const skills = resolveSkills(node.bundle.skills, options.skills.menu);

    /**
     * Keep the observation ledger local to this node. Appendix A, table A.2
     * requires every Observation call and return to remain correlated.
     */
    /** Compose the node-local agent with only its resolved executable tools. */
    const agent = createAgent({
      provider: options.provider,
      tools: createToolStorage(tools),
      messages,
      system: executionPrompt.system(options.skills.required),
      model: options.models.default,
    });

    options.logger.info(
      {
        nodeId: node.id,
        skills: node.bundle.skills,
        tools: tools.map(({ name }) => name),
      },
      'executing node',
    );

    /**
     * Run one agent loop with Mosaic's projected context and node-bound outcome
     * schema. Section 4.9 says the model may finish directly or call tools and
     * that an observation returns to the same node. `agent` always exposes the
     * outcome as a terminal tool, the paper's recommended provider-independent
     * profile.
     */
    const { structured: decision } = await agent.complete(
      executionPrompt.user({ request: input, node, graph, skills, tools }),
      {
        schema: createNodeDecisionSchema(node),
        maxTurns: options.execution.maxTurns,
      },
    );

    if (decision === undefined) {
      throw new Error(`Node ${node.id} returned no structured decision.`);
    }

    /**
     * MOSAIC 0.2 materializes the runtime outcome by attaching every correlated
     * executable-tool observation. The terminal structured-output tool is
     * normalized away by `agent` and therefore never enters this ledger.
     */
    const outcome = createNodeOutcomeSchema(node).parse({
      ...decision,
      observations: materializeObservations(node.id, messages.list()),
    });

    // Persist the complete semantic outcome before applying lifecycle policy.
    node.outcome = outcome;
    node.termination = null;

    if (outcome.status === 'needs_revision') {
      const request = outcome.revisionRequest;
      if (request === null) {
        throw new Error(`Node ${node.id} returned no revision request.`);
      }
      if (outcome.observations.length === 0) {
        throw new Error(
          `Node ${node.id} requested revision without an observation.`,
        );
      }

      node.status = outcome.status;
      options.logger.info(
        { nodeId: node.id, status: outcome.status },
        'node execution did not complete',
      );

      return;
    }

    /** Semantic blocked and failed decisions resolve the wave normally. */
    if (outcome.status !== 'completed') {
      node.status = outcome.status;
      options.logger.info(
        { nodeId: node.id, status: outcome.status },
        'node execution did not complete',
      );

      return;
    }

    const result = outcome.result;
    if (result === null) {
      throw new Error(`Node ${node.id} returned no completed result.`);
    }

    /**
     * Promote a completed result into the graph: Markdown is the primary
     * artifact and model-declared artifacts retain their original order.
     * Result promotion follows table A.2; `text/markdown` is Doric's concrete
     * artifact representation and is not prescribed by MOSAIC 0.2.
     */
    node.artifacts = [
      ...node.artifacts,
      { kind: 'inline', mime: 'text/markdown', data: result.markdown },
      ...result.artifacts,
    ];

    /** Completion makes the node eligible to satisfy downstream dependencies. */
    node.status = outcome.status;

    options.logger.info({ nodeId: node.id }, 'node execution completed');

    return;
  } catch (error) {
    if (
      error instanceof AgentErrorObject &&
      error.data.code === 'turn_limit_exceeded'
    ) {
      const observations = materializeObservations(node.id, messages.list());
      node.outcome = null;
      node.termination = {
        type: 'turn_limit',
        status: 'blocked',
        limit: options.execution.maxTurns,
        observations,
      };
      node.status = 'blocked';
      options.logger.info(
        { nodeId: node.id, status: node.status },
        'node execution did not complete',
      );
      return;
    }

    // Operational failures are not semantic outcomes and retain their identity.
    throw error;
  }
};

const executors = (
  node: Node,
  required: readonly Tool[],
  menu: readonly Tool[],
): Tool[] => {
  /** Required tools and the selected menu form the only executable catalog. */
  const catalog = new Map(
    [...required, ...menu].map((tool) => [tool.name, tool]),
  );
  const names = new Set<string>();

  /** Resolve in graph order while removing duplicate metadata references. */
  return node.tools.flatMap(({ name }) => {
    if (names.has(name)) return [];

    names.add(name);
    const tool = catalog.get(name);

    if (tool === undefined) {
      throw new Error(
        `Node ${node.id} references an unavailable tool: ${name}`,
      );
    }

    return [tool];
  });
};
