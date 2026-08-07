import type { Skill } from 'bundle';
import type { StateMachineHandler } from 'state-machine';

import * as bundlePrompt from '../../prompts/bundle.js';
import { createBundleSelectionSchema } from '../../schemas/bundle.js';
import type { Graph, Node } from '../../types/graph.js';
import type { NodeSkillSelection } from '../../types/node-skill-selection.js';
import type { WorkflowContext, WorkflowState } from '../../types/workflow.js';
import { composeTools, metadata, resolveSkills } from './menus.js';

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
};

/** Preparation inputs after vector retrieval produced canonical candidates. */
type CandidateSelection = Preparation & {
  readonly candidates: readonly Skill[];
};

/** Preparation inputs after the candidates received an authoritative order. */
type RankedSelection = Preparation & {
  readonly reranked: readonly Skill[];
};

/** Selects ordered skill references and derives the exact tool menu for a wave. */
export const bundle: StateMachineHandler<
  WorkflowContext,
  WorkflowState
> = async ({ graphs }, { input, options }, { transition, fail }) => {
  try {
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
      await prepare({ input, node, graph, options });
    }

    // Every ready node now has its selected skill references and exact tool menu.
    return transition('execution', { graphs });
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
}: Preparation): Promise<void> => {
  const { skills, tools, routing, logger } = options;

  // Stage 1: retrieve a bounded set of routable canonical skill definitions.
  const candidates = await retrieve({ input, node, graph, options });

  // Stage 2: an empty catalog or a zero limit produces a valid empty bundle.
  const selected =
    candidates.length === 0 || routing.maxSkills === 0
      ? []
      : await select({ input, node, graph, options, candidates });

  // Stage 3: persist lightweight references and derive tools from the catalog.
  assign({
    node,
    selected,
    skillMenu: skills.menu,
    requiredTools: tools.required,
    toolMenu: tools.menu,
  });

  // Log only stable identifiers; bodies, rationales, and artifacts stay private.
  logSelection(logger, node);
};

/** Builds the routing context and resolves vector matches to the current catalog. */
const retrieve = async ({
  input,
  node,
  graph,
  options,
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
  const matches = await skills.embeddings.search(
    context,
    routing.maxCandidates,
  );
  return currentCandidates(matches, catalog);
};

/** Applies the two model-assisted stages: deterministic reranking then selection. */
const select = async (
  input: CandidateSelection,
): Promise<NodeSkillSelection[]> => {
  const reranked = await rerank(input);
  return choose({ ...input, reranked });
};

/** Reranks complete candidate bodies and normalizes the provider response locally. */
const rerank = async ({
  input,
  node,
  graph,
  options,
  candidates,
}: CandidateSelection): Promise<Skill[]> => {
  const { provider, models } = options;

  // The reranker sees the full routing context and complete canonical skill bodies.
  const ranking = await provider.rerank({
    model: models.reranker,
    query: bundlePrompt.rerankQuery(input, node, graph),
    documents: candidates.map(bundlePrompt.candidateDocument),
    topN: candidates.length,
    flags: { sensitiveOutput: true },
  });

  // Provider order is not trusted; validation and sorting happen in this process.
  return orderCandidates(candidates, ranking);
};

/** Asks the selector for a bounded set of names with one rationale per skill. */
const choose = async ({
  input,
  node,
  graph,
  options,
  reranked,
}: RankedSelection): Promise<NodeSkillSelection[]> => {
  const { provider, models, routing } = options;

  // The node-bound schema rejects wrong goals, unknown names, duplicates, and overflow.
  const schema = createBundleSelectionSchema(
    node.id,
    reranked.map(({ name }) => name),
    routing.maxSkills,
  );

  // Candidate bodies and prior artifacts make this a sensitive structured call.
  const { structured } = await provider.complete({
    model: models.default,
    messages: [
      { role: 'system', content: bundlePrompt.system(routing.maxSkills) },
      {
        role: 'user',
        content: bundlePrompt.user({
          request: input,
          node,
          graph,
          skills: reranked,
        }),
      },
    ],
    schema,
    flags: { sensitiveOutput: true },
  });

  // Model output is a set; reranker order remains the observable bundle order.
  return normalize(reranked, structured.skills);
};

/** Drops universal, stale, and duplicate vector matches while preserving recall order. */
const currentCandidates = (
  matches: readonly { readonly data: Skill }[],
  catalog: ReadonlyMap<string, Skill>,
): Skill[] => {
  const seen = new Set<string>();

  return matches.flatMap(({ data }) => {
    const skill = catalog.get(data.name);
    if (skill === undefined || seen.has(skill.name)) return [];
    seen.add(skill.name);
    return [skill];
  });
};

/** Validates the ranking and sorts by score, then canonical name for stable ties. */
const orderCandidates = (
  candidates: readonly Skill[],
  ranking: readonly Ranking[],
): Skill[] => {
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
    .map(({ index }) => candidates[index]!);
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

/** Attaches rationales to the authoritative reranked order selected by the model. */
const normalize = (
  reranked: readonly Skill[],
  selected: readonly NodeSkillSelection[],
): NodeSkillSelection[] => {
  const byName = new Map(selected.map((item) => [item.skill, item]));
  return reranked.flatMap(({ name }) => byName.get(name) ?? []);
};

/** Inputs required to materialize the selected bundle on its graph node. */
type Assignment = {
  readonly node: Node;
  readonly selected: readonly NodeSkillSelection[];
  readonly skillMenu: readonly Skill[];
  readonly requiredTools: WorkflowContext['options']['tools']['required'];
  readonly toolMenu: WorkflowContext['options']['tools']['menu'];
};

/** Resolves definitions transiently, derives tools, and stores only compact state. */
const assign = ({
  node,
  selected,
  skillMenu,
  requiredTools,
  toolMenu,
}: Assignment): void => {
  // Full definitions live in the catalog; node.skills retains only name and rationale.
  const skills = resolveSkills(selected, skillMenu);

  // Tool visibility is exactly base tools plus allowed tools in selected-skill order.
  const tools = composeTools(skills, requiredTools, toolMenu);

  // Metadata is sufficient for graph state; executable tools remain in the catalog.
  node.skills = [...selected];
  node.tools = metadata(tools);
};

/** Emits the safe, identifier-only summary of the completed bundle stage. */
const logSelection = (
  logger: WorkflowContext['options']['logger'],
  node: Node,
): void => {
  logger.debug(
    {
      nodeId: node.id,
      skills: node.skills.map(({ skill }) => skill),
      tools: node.tools.map(({ name }) => name),
    },
    'bundle selected',
  );
};

/** Locale-independent code-point comparison used for deterministic tie-breaking. */
const compare = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;
