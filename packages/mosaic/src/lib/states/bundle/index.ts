import type { Skill } from 'bundle';
import * as bundlePrompt from '../../prompts/bundle.js';
import { createBundleSelectionSchema } from '../../schemas/bundle.js';
import {
  OrderedBundleSchema,
  SkillCandidateSchema,
} from '../../schemas/routing.js';
import { completeStructured } from '../../structured.js';
import { evaluate } from '../../evaluation.js';
import type { Graph, Node } from '../../types/graph.js';
import type { RoutingTrace } from '../../types/routing.js';
import type { MosaicEvaluationHooks } from '../../types/evaluation.js';
import type { MosaicRuntime } from '../../observability.js';
import type { WorkflowContext, WorkflowHandler } from '../../types/workflow.js';
import { composeTools, metadata, resolveSkills } from './menus.js';
import { validateMatches, validateTools, validateTrace } from './validation.js';

/** Provider ranking entry. The index points back to the retrieved candidate. */
type Ranking = {
  readonly index: number;
  readonly relevanceScore: number;
};

/** Inputs shared by every stage that prepares one ready node. */
type Preparation = {
  readonly input: string;
  readonly node: Node;
  readonly graph: Graph;
  readonly options: WorkflowContext['options'];
  readonly runtime: WorkflowContext['runtime'];
  readonly hooks: WorkflowContext['hooks'];
};

/** Preparation inputs after vector retrieval produced canonical candidates. */
type CandidateSelection = Preparation & {
  readonly candidates: readonly Skill[];
};

/** Preparation inputs after the candidates received an authoritative order. */
type RankedSelection = Preparation & {
  readonly reranked: readonly RankedCandidate[];
};

type RankedCandidate = {
  readonly skill: Skill;
  readonly score: number;
  readonly rank: number;
};

const NO_CANDIDATES_RATIONALE = 'No routable skill candidates were available.';
const ROUTING_DISABLED_RATIONALE =
  'Skill routing is disabled because maxSkills is zero.';

/** Selects ordered skill references and derives the exact tool menu for a wave. */
export const bundle: WorkflowHandler = async (
  state,
  { input, options, runtime, hooks },
  { transition, fail },
) => {
  try {
    const { graphs } = state;
    // Mosaic keeps graph revisions as a stack; only the newest graph is active.
    const graph = graphs.at(-1);
    if (graph === undefined) {
      return fail(new Error('Impossible to continue, missing active graph!'));
    }

    options.logger.info({}, 'generating bundles');

    // Scheduling marks the exact wave to prepare with the `ready` status.
    const nodes = graph.nodes.filter(({ status }) => status === 'ready');

    // Prepare nodes sequentially so provider calls and node mutations stay ordered.
    for (const node of nodes) {
      await prepare({ input, node, graph, options, runtime, hooks });
    }

    // Every ready node now has its selected skill references and exact tool menu.
    return transition('execution', state);
  } catch (error) {
    // Provider, catalog, and validation failures become workflow domain failures.
    return fail(error);
  }
};

/** Runs retrieval, optional model selection, and menu assignment for one node. */
const prepare = async ({
  input,
  node,
  graph,
  options,
  runtime,
  hooks,
}: Preparation): Promise<void> => {
  const { skills, tools, routing, logger } = options;

  // A zero selection limit bypasses every retrieval and model-assisted stage.
  const trace =
    routing.maxSkills === 0
      ? emptyTrace(node.id, ROUTING_DISABLED_RATIONALE)
      : await evaluate(
          hooks?.routing,
          { request: input, node, graph },
          async () => route({ input, node, graph, options, runtime, hooks }),
        );
  const validated = validateTrace(
    trace,
    node,
    skills.menu.filter(
      ({ name }) => !skills.required.some((required) => required.name === name),
    ),
    routing.maxRetrievedCandidates,
    routing.maxSkills,
  );

  // Persist the diagnostic trace and derive execution state from the bundle only.
  await assign({
    node,
    trace: validated,
    skillMenu: skills.menu,
    requiredTools: tools.required,
    toolMenu: tools.menu,
    revision: graph.revision,
    runtime,
    hook: hooks?.menu,
  });

  await runtime?.emit({
    type: 'bundle.selected',
    stage: 'bundle',
    nodeId: node.id,
    revision: graph.revision,
    candidateNames: node.candidates.map(({ skillName }) => skillName),
    skillNames: node.bundle?.skills ?? [],
    ...(runtime.capture === 'io'
      ? { candidates: node.candidates, bundle: node.bundle }
      : {}),
  });

  // Log only stable identifiers; bodies, rationales, and artifacts stay private.
  logSelection(logger, node);
};

/** Retrieves candidates and materializes either a deterministic or selected trace. */
const route = async (input: Preparation): Promise<RoutingTrace> => {
  const candidates = await retrieve(input);
  if (candidates.length === 0) {
    return emptyTrace(input.node.id, NO_CANDIDATES_RATIONALE);
  }
  return select({ ...input, candidates });
};

/** Builds the routing context and resolves vector matches to the current catalog. */
const retrieve = async ({
  input,
  node,
  graph,
  options,
  runtime,
  hooks,
}: Preparation): Promise<Skill[]> => {
  const { skills, routing } = options;

  // The same request, node, criteria, and ancestor artifacts feed every route stage.
  const context = bundlePrompt.routingContext(input, node, graph);

  // Universal skills apply outside B(g), so they cannot become routed candidates.
  const required = new Set(skills.required.map(({ name }) => name));

  // Catalog lookup replaces stale indexed objects with the current definitions.
  const catalog = new Map(
    skills.menu
      .filter(({ name }) => !required.has(name))
      .map((skill) => [skill.name, skill]),
  );

  // Vector retrieval limits recall independently from the final bundle limit.
  const matches = await evaluate(
    hooks?.retrieval,
    {
      request: input,
      graph,
      node,
      query: context,
      limit: routing.maxRetrievedCandidates,
      catalog: [...catalog.values()],
    },
    async ({ query, limit }) =>
      (await skills.retriever.search(query, limit)).map(
        ({ data: skill, score }) => ({ skill, score }),
      ),
  );
  const candidates = currentCandidates(
    validateMatches(matches, catalog, routing.maxRetrievedCandidates).map(
      ({ skill }) => ({ data: skill }),
    ),
    catalog,
    routing.maxRetrievedCandidates,
  );
  await runtime?.emit({
    type: 'retrieval.result',
    stage: 'bundle',
    nodeId: node.id,
    revision: graph.revision,
    skillNames: candidates.map(({ name }) => name),
    ...(runtime?.capture === 'io' ? { query: context } : {}),
  });
  return candidates;
};

/** Applies the two model-assisted stages: deterministic reranking then selection. */
const select = async (input: CandidateSelection): Promise<RoutingTrace> => {
  const candidates = await Promise.all(
    input.candidates.map(async (skill) => {
      const body = await evaluate(
        input.hooks?.skillView,
        { skill, purpose: 'routing' as const, nodeId: input.node.id },
        async ({ skill: current }) => current.body,
      );
      if (body.trim().length === 0) {
        throw new Error('Evaluation skill view must be non-empty.');
      }
      return { ...skill, body };
    }),
  );
  const reranked = await rerank({ ...input, candidates });
  return choose({ ...input, reranked });
};

/** Reranks complete candidate bodies and normalizes the provider response locally. */
const rerank = async ({
  input,
  node,
  graph,
  options,
  candidates,
  runtime,
}: CandidateSelection): Promise<RankedCandidate[]> => {
  const { providers, models } = options;

  // The reranker sees the full routing context and complete canonical skill bodies.
  const { results: ranking } = await (
    runtime?.provider(providers.reranker, 'bundle', node.id, graph.revision) ??
    providers.reranker
  ).rerank({
    model: models.reranker,
    query: bundlePrompt.rerankQuery(input, node, graph),
    documents: candidates.map(bundlePrompt.candidateDocument),
    topN: candidates.length,
    flags: { sensitiveOutput: true },
  });

  // Provider order is not trusted; validation and sorting happen in this process.
  const ordered = orderCandidates(candidates, ranking);
  await runtime?.emit({
    type: 'rerank.result',
    stage: 'bundle',
    nodeId: node.id,
    revision: graph.revision,
    ranking: ordered.map(({ skill, score, rank }) => ({
      skillName: skill.name,
      score,
      rank,
    })),
  });
  return ordered;
};

/** Asks the selector for a bounded set of names with one rationale per skill. */
const choose = async ({
  input,
  node,
  graph,
  options,
  reranked,
  runtime,
}: RankedSelection): Promise<RoutingTrace> => {
  const { providers, models, routing } = options;

  // The node-bound schema rejects wrong goals, unknown names, duplicates, and overflow.
  const schema = createBundleSelectionSchema(
    node.id,
    reranked.map(({ skill }) => skill.name),
    routing.maxSkills,
  );

  // Candidate bodies and prior artifacts make this a sensitive structured call.
  const structured = await completeStructured({
    provider: providers.planning,
    profile: models.planning,
    system: bundlePrompt.system(routing.maxSkills),
    input: bundlePrompt.user({
      request: input,
      node,
      graph,
      skills: reranked.map(({ skill }) => skill),
    }),
    schema,
    flags: { sensitiveOutput: true },
    runtime,
    stage: 'bundle',
    nodeId: node.id,
    revision: graph.revision,
  });

  const evaluations = new Map(
    structured.evaluations.map((evaluation) => [
      evaluation.skillName,
      evaluation,
    ]),
  );
  const candidates = SkillCandidateSchema.array().parse(
    reranked.map(({ skill, score, rank }) => ({
      skillName: skill.name,
      score,
      rank,
      rationale: evaluations.get(skill.name)!.rationale,
    })),
  );
  const selected = new Set(
    structured.evaluations
      .filter(({ selected }) => selected)
      .map(({ skillName }) => skillName),
  );
  const bundle = OrderedBundleSchema.parse({
    goalId: node.id,
    skills: candidates.flatMap(({ skillName }) =>
      selected.has(skillName) ? [skillName] : [],
    ),
    selectionRationale: structured.selectionRationale,
  });

  return { candidates, bundle };
};

/** Drops universal, stale, and duplicate vector matches while preserving recall order. */
const currentCandidates = (
  matches: readonly { readonly data: Skill }[],
  catalog: ReadonlyMap<string, Skill>,
  limit: number,
): Skill[] => {
  const seen = new Set<string>();

  return matches
    .flatMap(({ data }) => {
      const skill = catalog.get(data.name);
      if (skill === undefined || seen.has(skill.name)) return [];
      seen.add(skill.name);
      return [skill];
    })
    .slice(0, limit);
};

/** Validates the ranking and sorts by score, then canonical name for stable ties. */
const orderCandidates = (
  candidates: readonly Skill[],
  ranking: readonly Ranking[],
): RankedCandidate[] => {
  validateRanking(ranking, candidates.length);

  return [...ranking]
    .sort((left, right) => {
      const score = right.relevanceScore - left.relevanceScore;
      if (score !== 0) return score;
      return compare(
        candidates[left.index]!.name,
        candidates[right.index]!.name,
      );
    })
    .map(({ index, relevanceScore }, position) => ({
      skill: candidates[index]!,
      score: relevanceScore,
      rank: position + 1,
    }));
};

/** Enforces the one-result-per-candidate contract before any index is dereferenced. */
const validateRanking = (
  ranking: readonly Ranking[],
  candidateCount: number,
): void => {
  if (ranking.length !== candidateCount) {
    throw new Error('Reranker returned an incomplete candidate ranking.');
  }

  const indices = new Set<number>();
  for (const result of ranking) {
    const validIndex =
      Number.isSafeInteger(result.index) &&
      result.index >= 0 &&
      result.index < candidateCount;

    if (!validIndex || indices.has(result.index)) {
      throw new Error('Reranker returned an invalid candidate index.');
    }

    if (!Number.isFinite(result.relevanceScore)) {
      throw new Error('Reranker returned an invalid relevance score.');
    }

    indices.add(result.index);
  }
};

/** Inputs required to materialize the selected bundle on its graph node. */
type Assignment = {
  readonly node: Node;
  readonly trace: RoutingTrace;
  readonly skillMenu: readonly Skill[];
  readonly requiredTools: WorkflowContext['options']['tools']['required'];
  readonly toolMenu: WorkflowContext['options']['tools']['menu'];
  readonly revision: number;
  readonly runtime?: MosaicRuntime;
  readonly hook?: MosaicEvaluationHooks['menu'];
};

/** Resolves definitions transiently, derives tools, and stores only compact state. */
const assign = async ({
  node,
  trace,
  skillMenu,
  requiredTools,
  toolMenu,
  revision,
  runtime,
  hook,
}: Assignment): Promise<void> => {
  // Full definitions live in the catalog and resolve only from bundle names.
  const skills = resolveSkills(trace.bundle.skills, skillMenu);

  // Tool visibility is exactly base tools plus allowed tools in selected-skill order.
  const tools = await evaluate(
    hook,
    {
      node,
      trace,
      skills,
      required: requiredTools,
      catalog: toolMenu,
    },
    async ({ skills: selected, required, catalog }) =>
      composeTools(selected, required, catalog),
  );
  const validatedTools = validateTools(tools, requiredTools, toolMenu);

  // Metadata is sufficient for the graph snapshot; executable tools remain in the catalog.
  node.candidates = trace.candidates.map((candidate) => ({ ...candidate }));
  node.bundle = {
    ...trace.bundle,
    skills: [...trace.bundle.skills],
  };
  node.tools = metadata(validatedTools);
  await runtime?.emit({
    type: 'menu.composed',
    stage: 'bundle',
    nodeId: node.id,
    revision,
    toolNames: validatedTools.map(({ name }) => name),
  });
};

const emptyTrace = (
  goalId: string,
  selectionRationale: string,
): RoutingTrace => ({
  candidates: [],
  bundle: { goalId, skills: [], selectionRationale },
});

/** Emits the safe, identifier-only summary of the completed bundle stage. */
const logSelection = (
  logger: WorkflowContext['options']['logger'],
  node: Node,
): void => {
  logger.debug(
    {
      nodeId: node.id,
      skills: node.bundle?.skills ?? [],
      tools: node.tools.map(({ name }) => name),
    },
    'bundle selected',
  );
};

/** Locale-independent code-point comparison used for deterministic tie-breaking. */
const compare = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;
