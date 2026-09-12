import type { Skill } from 'bundle';

import { evaluate } from '../../evaluation.js';
import type { MosaicRuntime } from '../../observability.js';
import * as candidatesPrompt from '../../prompts/candidates.js';
import * as hintsPrompt from '../../prompts/hints.js';
import { SkillHintExtractionSchema } from '../../schemas/hint.js';
import { completeStructured } from '../../structured.js';
import type {
  MosaicEvaluationHooks,
  SkillMatch,
} from '../../types/evaluation.js';
import type { Graph } from '../../types/graph.js';
import type { SkillExtraction } from '../../types/hint.js';
import type { MosaicOptions } from '../../types/mosaic-options.js';

/**
 * Produces the catalog feedback described in section 4.5, preserving
 * node and candidate order while bounding retrieval independently for each goal.
 */
export async function hints(
  input: string,
  graph: Graph,
  options: MosaicOptions,
  runtime?: MosaicRuntime,
  hooks?: MosaicEvaluationHooks,
): Promise<SkillExtraction[]> {
  const { providers, skills, models } = options;
  const required = new Set(skills.required.map(({ name }) => name));

  // Resolve indexed matches through the canonical, currently loaded catalog.
  const catalog = new Map(
    skills.menu
      .filter(({ name }) => !required.has(name))
      .map((skill) => [skill.name, skill]),
  );

  // An entirely empty catalog contributes no evidence and needs no model calls.
  if (skills.required.length === 0 && catalog.size === 0) {return [];}

  // Goals are independent at this stage, so their preliminary retrieval can run together.
  const byNode = await Promise.all(
    graph.nodes.map(async (node) => {
      const optional =
        catalog.size === 0
          ? []
          : await retrieveCandidates(
              input,
              graph,
              node,
              options,
              catalog,
              runtime,
              hooks,
            );
      // Required skills are evaluated first and do not consume K_hint.
      const candidates = [...skills.required, ...optional];

      // Convert each complete skill body into short, goal-specific planning hints.
      const extracted = await Promise.all(
        candidates.map(async (skill) => {
          const body = await evaluate(
            hooks?.skillView,
            { skill, purpose: 'hint' as const, nodeId: node.id },
            async ({ skill: current }) => current.body,
          );

          if (body.trim().length === 0) {
            throw new Error('Evaluation skill view must be non-empty.');
          }

          const viewed = { ...skill, body };

          const structured = await completeStructured({
            provider: providers.planning,
            profile: models.planning,
            system: hintsPrompt.system(),
            input: hintsPrompt.user(input, graph, node, viewed),
            schema: SkillHintExtractionSchema,
            runtime,
            stage: 'plan',
            nodeId: node.id,
            revision: graph.revision,
          });

          await runtime?.emit({
            type: 'hint.result',
            stage: 'plan',
            nodeId: node.id,
            revision: graph.revision,
            skillName: skill.name,
            hintCount: structured.hints.length,
            ...(runtime.capture === 'io' ? { hints: structured.hints } : {}),
          });

          // A skill with no useful hint is omitted instead of influencing P1.
          return structured.hints.length === 0
            ? undefined
            : { goalId: node.id, skill, hints: structured.hints };
        }),
      );

      return extracted.filter(
        (extraction): extraction is SkillExtraction => extraction !== undefined,
      );
    }),
  );

  // Flatten only after all work completes to retain graph and candidate ordering.
  return byNode.flat();
}

const retrieveCandidates = async (
  input: string,
  graph: Graph,
  node: Graph['nodes'][number],
  options: MosaicOptions,
  catalog: ReadonlyMap<string, Skill>,
  runtime?: MosaicRuntime,
  hooks?: MosaicEvaluationHooks,
): Promise<Skill[]> => {
  const { routing, skills } = options;
  const query = candidatesPrompt.search(input, node);

  const matches = await evaluate(
    hooks?.retrieval,
    {
      request: input,
      graph,
      node,
      query,
      limit: routing.maxHintCandidates,
      catalog: [...catalog.values()],
    },
    async ({ query: searchQuery, limit }) =>
      (await skills.retriever.search(searchQuery, limit)).map(
        ({ data: skill, score }) => ({ skill, score }),
      ),
  );

  const candidates = canonicalCandidates(
    validateMatches(matches, catalog, routing.maxHintCandidates).map(
      ({ skill }) => skill,
    ),
    catalog,
    routing.maxHintCandidates,
  );

  await runtime?.emit({
    type: 'retrieval.result',
    stage: 'plan',
    nodeId: node.id,
    revision: graph.revision,
    skillNames: candidates.map(({ name }) => name),
    ...(runtime.capture === 'io' ? { query } : {}),
  });

  return candidates;
};

/** Keeps only current, unique candidates and enforces the defensive hint bound. */
const canonicalCandidates = (
  matches: readonly Skill[],
  catalog: ReadonlyMap<string, Skill>,
  limit: number,
): Skill[] => {
  const seen = new Set<string>();

  return matches
    .flatMap((match) => {
      // Indexed definitions may be stale or duplicated; only catalog identity survives.
      const skill = catalog.get(match.name);

      if (skill === undefined || seen.has(skill.name)) {return [];}

      seen.add(skill.name);

      return [skill];
    })
    .slice(0, limit);
};

const validateMatches = (
  matches: readonly SkillMatch[],
  catalog: ReadonlyMap<string, Skill>,
  limit: number,
): readonly SkillMatch[] => {
  const seen = new Set<string>();

  return matches
    .flatMap(({ skill, score }) => {
      if (!Number.isFinite(score)) {
        throw new Error(
          'Evaluation retrieval returned an invalid skill score.',
        );
      }

      const current = catalog.get(skill.name);

      if (current === undefined || seen.has(current.name)) {return [];}

      seen.add(current.name);

      return [{ skill: current, score }];
    })
    .slice(0, limit);
};
