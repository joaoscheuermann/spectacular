import { AgentErrorObject, createAgent } from 'agent';
import { createMessageStorage } from 'messages';
import {
  createToolStorage,
  type Tool,
  type ToolCall,
  type ToolCallRequest,
  type ToolStorage,
} from 'tool';

import * as executionPrompt from '../../prompts/execution.js';
import {
  createNodeDecisionSchema,
  createNodeOutcomeSchema,
} from '../../schemas/outcome.js';
import { resolveSkills } from '../bundle/menus.js';
import type { Graph, Node } from '../../types/graph.js';
import type { WorkflowContext, WorkflowHandler } from '../../types/workflow.js';
import { materializeObservations } from './observations.js';
import { evaluate } from '../../evaluation.js';
import type { MosaicRuntime } from '../../observability.js';
import type { MosaicEvaluationHooks } from '../../types/evaluation.js';

/**
 * Implements the node executor and lifecycle from the MOSAIC paper, sections
 * 4.9-4.10 and Algorithm 1. In particular, section 4.10
 * says: "O scheduler o coloca em running." Doric groups ready nodes into a
 * concurrent wave; that batching policy is a local MOSAIC 0.2 runtime choice.
 */
export const execution: WorkflowHandler = async (
  state,
  { input, options, runtime, hooks },
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
    await runtime?.emit({
      type: 'wave.started',
      stage: 'execution',
      revision: graph.revision,
      nodeIds: nodes.map(({ id }) => id),
    });
    const results = await Promise.allSettled(
      nodes.map((node) => execute(input, node, graph, options, runtime, hooks)),
    );

    const failure = results.find(
      (result): result is PromiseRejectedResult => result.status === 'rejected',
    );

    if (failure !== undefined) return fail(failure.reason);

    await runtime?.emit({
      type: 'wave.finished',
      stage: 'execution',
      revision: graph.revision,
      nodeIds: nodes.map(({ id }) => id),
    });

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
  runtime?: MosaicRuntime,
  hooks?: MosaicEvaluationHooks,
): Promise<void> => {
  /** The runtime owns node status after scheduling hands the node to execution. */
  node.status = 'running';
  await runtime?.emit({
    type: 'node.status',
    stage: 'execution',
    revision: graph.revision,
    nodeId: node.id,
    status: node.status,
  });
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
    const execution = await evaluate(
      hooks?.execution,
      { request: input, node, graph, skills, tools },
      async ({ request, node: current, graph: active, skills, tools }) => {
        if (current.id !== node.id || active.revision !== graph.revision) {
          throw new Error('Evaluation execution changed its node identity.');
        }
        const agent = createAgent({
          provider:
            runtime?.provider(
              options.providers.execution,
              'execution',
              node.id,
              graph.revision,
            ) ?? options.providers.execution,
          tools: observedTools(tools, node.id, graph.revision, runtime),
          messages,
          system: executionPrompt.system(options.skills.required),
          model: options.models.execution.model,
          effort: options.models.execution.effort,
        });
        const { structured: decision } = await agent.complete(
          executionPrompt.user({
            request,
            node: current,
            graph: active,
            skills,
            tools,
          }),
          {
            schema: createNodeDecisionSchema(node),
            maxTurns: options.execution.maxTurns,
            ...(runtime === undefined
              ? {}
              : {
                  onStructuredAttempt: (event) =>
                    runtime.emit({
                      type: 'structured.attempt',
                      providerId: options.providers.execution.metadata.id,
                      stage: 'execution',
                      nodeId: node.id,
                      revision: graph.revision,
                      attempt: event.attempt,
                      runtimeAccepted: event.runtimeAccepted,
                      feedbackSent: event.feedbackSent,
                      ...(event.diagnostic === undefined
                        ? {}
                        : { diagnostic: event.diagnostic }),
                    }),
                  onToolCallRepair: (event) =>
                    runtime.emit({
                      type: 'tool.repair',
                      providerId: options.providers.execution.metadata.id,
                      stage: 'execution',
                      nodeId: node.id,
                      revision: graph.revision,
                      attempt: event.attempt,
                      maxAttempts: event.maxAttempts,
                    }),
                }),
          },
        );
        if (decision === undefined) {
          throw new Error(`Node ${node.id} returned no structured decision.`);
        }
        return {
          decision,
          observations: materializeObservations(node.id, messages.list()),
        };
      },
    );
    const decision = createNodeDecisionSchema(node).parse(execution.decision);

    /**
     * MOSAIC 0.2 materializes the runtime outcome by attaching every correlated
     * executable-tool observation. The terminal structured-output tool is
     * normalized away by `agent` and therefore never enters this ledger.
     */
    const outcome = createNodeOutcomeSchema(node).parse({
      ...decision,
      observations: execution.observations,
    });

    await runtime?.emit({
      type: 'decision.created',
      stage: 'execution',
      nodeId: node.id,
      revision: graph.revision,
      status: decision.status,
      ...(runtime.capture === 'io' ? { decision } : {}),
    });
    await runtime?.emit({
      type: 'observations.created',
      stage: 'execution',
      nodeId: node.id,
      revision: graph.revision,
      count: outcome.observations.length,
      toolNames: outcome.observations.map(({ toolName }) => toolName),
      ...(runtime.capture === 'io'
        ? { observations: outcome.observations }
        : {}),
    });
    await runtime?.emit({
      type: 'outcome.created',
      stage: 'execution',
      nodeId: node.id,
      revision: graph.revision,
      status: outcome.status,
      ...(runtime.capture === 'io' ? { outcome } : {}),
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
      await statusEvent(runtime, graph, node);
      options.logger.info(
        { nodeId: node.id, status: outcome.status },
        'node execution did not complete',
      );

      return;
    }

    /** Semantic blocked and failed decisions resolve the wave normally. */
    if (outcome.status !== 'completed') {
      node.status = outcome.status;
      await statusEvent(runtime, graph, node);
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
    await statusEvent(runtime, graph, node);

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
      await statusEvent(runtime, graph, node);
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

const observedTools = (
  tools: readonly Tool[],
  nodeId: string,
  revision: number,
  runtime?: MosaicRuntime,
): ToolStorage => {
  const storage = createToolStorage(tools);
  if (runtime === undefined) return storage;

  return {
    definitions: () => storage.definitions(),
    calls: (turn) => storage.calls(turn),
    validate: (call) => storage.validate(call),
    get: (name) => storage.get(name),
    execute: async (call) => {
      const identity = callIdentity(call);
      await runtime.emit({
        type: 'tool.started',
        stage: 'execution',
        nodeId,
        revision,
        ...identity,
        ...(runtime.capture === 'io' ? { input: callInput(call) } : {}),
      });
      const timer = runtime.timer();
      try {
        const output = await storage.execute(call);
        await runtime.emit({
          type: 'tool.finished',
          stage: 'execution',
          nodeId,
          revision,
          ...identity,
          durationMs: runtime.duration(timer),
          ...(runtime.capture === 'io' ? { output } : {}),
        });
        return output;
      } catch (error) {
        await runtime.emit({
          type: 'tool.failed',
          stage: 'execution',
          nodeId,
          revision,
          ...identity,
          durationMs: runtime.duration(timer),
        });
        throw error;
      }
    },
  };
};

const callIdentity = (call: ToolCall | ToolCallRequest) => ({
  callId: call.id,
  toolName: call.name,
});

const callInput = (call: ToolCall | ToolCallRequest): unknown => {
  if ('payload' in call) return call.payload;
  try {
    return JSON.parse(call.arguments) as unknown;
  } catch {
    return undefined;
  }
};

const statusEvent = async (
  runtime: MosaicRuntime | undefined,
  graph: Graph,
  node: Node,
): Promise<void> => {
  await runtime?.emit({
    type: 'node.status',
    stage: 'execution',
    revision: graph.revision,
    nodeId: node.id,
    status: node.status,
  });
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
