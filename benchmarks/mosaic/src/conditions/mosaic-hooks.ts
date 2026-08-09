import type {
  ExecutionResult,
  MosaicEvaluationHooks,
  RoutingInput,
} from 'mosaic/evaluation';

import type { Case, Condition } from '../schemas/index.js';
import { TOOL_NAMES } from '../config/index.js';
import type { JsonValue } from '../core/json.js';
import { createPrng } from '../core/prng.js';
import type { RunContext } from '../runtime/index.js';
import {
  mosaicSkills,
  type MosaicDependencies,
  type MosaicOracleDependencies,
} from './mosaic-adapters.js';

const oraclePlan = (benchmarkCase: Case) => ({
  nodes: [
    {
      id: 'g01:oracle',
      goal: benchmarkCase.request,
      doneWhen: benchmarkCase.gold.criteria.map(
        (criterion) => criterion.description,
      ),
      dependsOn: [],
      deliver: true,
    },
  ],
});

const shuffledRouting = async (
  input: Readonly<RoutingInput>,
  next: (
    value: Readonly<RoutingInput>,
  ) => Promise<
    Awaited<ReturnType<NonNullable<MosaicEvaluationHooks['routing']>>>
  >,
  seed: string,
) => {
  const trace = await next(input);
  const candidates = createPrng(seed)
    .shuffle(trace.candidates)
    .map((candidate, index) => ({ ...candidate, rank: index + 1 }));
  const selected = new Set(trace.bundle.skills);
  return {
    candidates,
    bundle: {
      ...trace.bundle,
      skills: candidates.flatMap((candidate) =>
        selected.has(candidate.skillName) ? [candidate.skillName] : [],
      ),
    },
  };
};

const topKRouting = async (
  input: Readonly<RoutingInput>,
  limit: number,
  dependencies: MosaicDependencies,
  startModelCall: () => Promise<void>,
  emit: (event: JsonValue) => Promise<void>,
) => {
  const matches = await dependencies.retrievers.skills.search(
    `${input.request}\n${input.node.goal}\n${input.node.doneWhen.join('\n')}`,
    dependencies.base.routing.maxRetrievedCandidates,
  );
  const catalog = new Map(mosaicSkills().map((skill) => [skill.name, skill]));
  const selected = matches
    .flatMap((match) => catalog.get(match.data.name) ?? [])
    .slice(0, dependencies.base.routing.maxRetrievedCandidates);
  await emit({
    type: 'retrieval.result',
    stage: 'bundle',
    nodeId: input.node.id,
    revision: input.graph.revision,
    k: dependencies.base.routing.maxRetrievedCandidates,
    skillNames: selected.map(({ name }) => name),
  });
  if (selected.length === 0)
    return {
      candidates: [],
      bundle: {
        goalId: input.node.id,
        skills: [],
        selectionRationale: 'Fixed top-k empty bundle.',
      },
    };
  await startModelCall();
  await emit({
    type: 'model.request',
    stage: 'bundle',
    operation: 'rerank',
    model: dependencies.base.models.reranker,
    messageCount: selected.length,
  });
  const started = Date.now();
  let ranking;
  try {
    ranking = await dependencies.base.provider.rerank({
      model: dependencies.base.models.reranker,
      query: `${input.request}\n${input.node.goal}`,
      documents: selected.map((skill) => skill.body),
      topN: selected.length,
      flags: { sensitiveOutput: true },
    });
    await emit({
      type: 'model.response',
      stage: 'bundle',
      operation: 'rerank',
      model: dependencies.base.models.reranker,
      resultCount: ranking.length,
      durationMs: Math.max(0, Date.now() - started),
    });
  } catch (error) {
    await emit({
      type: 'model.failed',
      stage: 'bundle',
      operation: 'rerank',
      model: dependencies.base.models.reranker,
      status: 'failed',
      durationMs: Math.max(0, Date.now() - started),
    });
    throw error;
  }
  if (ranking.length !== selected.length)
    throw new TypeError('A4 reranker returned an incomplete ranking');
  const ordered = [...ranking]
    .sort(
      (left, right) =>
        right.relevanceScore - left.relevanceScore || left.index - right.index,
    )
    .map((entry, index) => ({
      skillName: selected[entry.index]?.name ?? '',
      score: entry.relevanceScore,
      rank: index + 1,
      rationale: 'Fixed top-k reranker order.',
    }));
  if (ordered.some((candidate) => candidate.skillName === ''))
    throw new TypeError('A4 reranker returned an invalid index');
  return {
    candidates: ordered,
    bundle: {
      goalId: input.node.id,
      skills: ordered.slice(0, limit).map((candidate) => candidate.skillName),
      selectionRationale: 'Fixed top-k diagnostic bundle.',
    },
  };
};

/** Maps M0/M1, A1-A5, and failure-only oracles to public evaluation hooks. */
export const evaluationHooks = (
  condition: Condition,
  benchmarkCase: Case,
  seed: number,
  oracles: MosaicOracleDependencies = {},
  dependencies?: MosaicDependencies,
  startModelCall: () => Promise<void> = async () => undefined,
  emit: (event: JsonValue) => Promise<void> = async () => undefined,
  context?: RunContext,
): MosaicEvaluationHooks => {
  let stateEvidenceApplied = false;
  const m0: MosaicEvaluationHooks =
    condition.id === 'M0' ? { feedbackPlan: async () => 'unchanged' } : {};
  const a1: MosaicEvaluationHooks =
    condition.id === 'A1'
      ? {
          skillView: async ({ skill }) =>
            `# ${skill.name}\n\n${skill.description}\n\nAllowed tools: ${skill.allowedTools.join(', ') || 'none'}.`,
          retrieval: async (input) => {
            if (dependencies === undefined)
              throw new TypeError('A1 requires an injected metadata retriever');
            const catalog = new Map(
              input.catalog.map((skill) => [skill.name, skill]),
            );
            const matches = await dependencies.retrievers.metadataSkills.search(
              input.query,
              input.limit,
            );
            return matches
              .flatMap((match) => {
                const skill = catalog.get(match.data.name);
                return skill === undefined
                  ? []
                  : [{ skill, score: match.score }];
              })
              .slice(0, input.limit);
          },
        }
      : {};
  const a2: MosaicEvaluationHooks =
    condition.id === 'A2'
      ? {
          routing: (input, next) =>
            shuffledRouting(input, next, `${seed}:${input.node.id}`),
        }
      : {};
  const a3: MosaicEvaluationHooks =
    condition.id === 'A3'
      ? { menu: (input, next) => next({ ...input, required: [] }) }
      : {};
  const a4: MosaicEvaluationHooks =
    condition.id === 'A4'
      ? {
          routing: (input) => {
            if (dependencies === undefined)
              throw new TypeError(
                'A4 requires injected retrievers and reranker',
              );
            return topKRouting(
              input,
              condition.factors.maxSkills ?? 3,
              dependencies,
              startModelCall,
              emit,
            );
          },
        }
      : {};
  const plan: MosaicEvaluationHooks =
    condition.id === 'O_PLAN'
      ? { initialPlan: async () => oraclePlan(benchmarkCase) }
      : {};
  const retrieval: MosaicEvaluationHooks =
    condition.id === 'O_RETRIEVAL'
      ? {
          retrieval: async (input) => {
            const order = [
              ...new Set([
                ...benchmarkCase.gold.requiredSkills,
                ...benchmarkCase.gold.relevantSkills,
              ]),
            ];
            const rank = new Map(order.map((name, index) => [name, index]));
            return input.catalog
              .filter((skill) => rank.has(skill.name))
              .sort(
                (left, right) =>
                  (rank.get(left.name) ?? 0) - (rank.get(right.name) ?? 0),
              )
              .map((skill, index) => ({
                skill,
                score: 1 - index / Math.max(1, input.catalog.length),
              }))
              .slice(0, input.limit);
          },
        }
      : {};
  const bundle: MosaicEvaluationHooks =
    condition.id === 'O_BUNDLE'
      ? {
          routing: async (input, next) => {
            const trace = await next(input);
            const gold = [...new Set(benchmarkCase.gold.requiredSkills)];
            const present = new Set(
              trace.candidates.map((candidate) => candidate.skillName),
            );
            const missing = gold
              .filter((name) => !present.has(name))
              .map((skillName) => ({
                skillName,
                score: 1,
                rank: 0,
                rationale: 'Injected missing gold skill.',
              }));
            const candidates = [...trace.candidates, ...missing].map(
              (candidate, index) => ({ ...candidate, rank: index + 1 }),
            );
            return {
              candidates,
              bundle: {
                ...trace.bundle,
                skills: gold,
                selectionRationale: 'Gold failure-diagnostic bundle.',
              },
            };
          },
        }
      : {};
  const menu: MosaicEvaluationHooks =
    condition.id === 'O_MENU'
      ? {
          menu: async (input) => {
            const gold = new Set(benchmarkCase.gold.requiredTools);
            return input.catalog.filter((tool) => gold.has(tool.name));
          },
        }
      : {};
  const state: MosaicEvaluationHooks =
    condition.id === 'O_STATE'
      ? {
          execution: async (input) => {
            if (context === undefined)
              throw new TypeError(
                'state oracle requires the run-local World context',
              );
            const observations: ExecutionResult['observations'][number][] = [];
            if (!stateEvidenceApplied) {
              stateEvidenceApplied = true;
              for (const [
                index,
                evidence,
              ] of benchmarkCase.gold.expectedState.toolEvidence.entries()) {
                if (!(TOOL_NAMES as readonly string[]).includes(evidence.name))
                  throw new TypeError(
                    'state oracle references an unknown tool',
                  );
                const callId = `oracle-${String(index + 1).padStart(3, '0')}`;
                const output = await context.callTool(
                  evidence.name as (typeof TOOL_NAMES)[number],
                  evidence.input as JsonValue,
                  {
                    callId,
                    nodeId: input.node.id,
                    revision: input.graph.revision,
                  },
                );
                observations.push({
                  goalId: input.node.id,
                  toolName: evidence.name,
                  callId,
                  input: JSON.stringify(evidence.input),
                  output: JSON.stringify(output),
                });
              }
            }
            if (oracles.state !== undefined) {
              const result = await oracles.state(input, benchmarkCase, context);
              return { ...result, observations };
            }
            return {
              decision: {
                status: 'completed',
                criteria: input.node.doneWhen.map(
                  (criterion, criterionIndex) => ({
                    criterionIndex,
                    satisfied: true,
                    evidence: `State oracle satisfied: ${criterion}`,
                  }),
                ),
                result: {
                  markdown: JSON.stringify(
                    benchmarkCase.gold.expectedDelivery.document,
                  ),
                  artifacts: [],
                },
                revisionRequest: null,
                reason: null,
              },
              observations,
            };
          },
        }
      : {};
  const revision: MosaicEvaluationHooks =
    condition.id === 'O_REVISION'
      ? {
          localizedRevision: async (input, next) =>
            oracles.revision === undefined
              ? next(input)
              : oracles.revision(input, benchmarkCase),
        }
      : {};
  return {
    ...m0,
    ...a1,
    ...a2,
    ...a3,
    ...a4,
    ...plan,
    ...retrieval,
    ...bundle,
    ...menu,
    ...state,
    ...revision,
  };
};
