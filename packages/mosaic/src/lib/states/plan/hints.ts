import type { Skill } from 'bundle';

import * as candidatesPrompt from '../../prompts/candidates.js';
import * as hintsPrompt from '../../prompts/hints.js';
import { SkillHintExtractionSchema } from '../../schemas/hint.js';
import type { Graph } from '../../types/graph.js';
import type { SkillExtraction } from '../../types/hint.js';
import type { MosaicOptions } from '../../types/mosaic-options.js';

/**
 * Produces the catalog feedback described in section 4.4 (p. 13), preserving
 * node and candidate order while bounding retrieval independently for each goal.
 */
export async function hints(
  input: string,
  graph: Graph,
  options: MosaicOptions,
): Promise<SkillExtraction[]> {
  const { provider, skills, models, routing } = options;

  // Always-available skills are universal instructions, not planning signals.
  const required = new Set(skills.required.map(({ name }) => name));

  // Resolve indexed matches through the canonical, currently loaded catalog.
  const catalog = new Map(
    skills.menu
      .filter(({ name }) => !required.has(name))
      .map((skill) => [skill.name, skill]),
  );

  // An empty routable catalog contributes no evidence and needs no model calls.
  if (catalog.size === 0) return [];

  // Goals are independent at this stage, so their preliminary retrieval can run together.
  const byNode = await Promise.all(
    graph.nodes.map(async (node) => {
      // Retrieve at most K_hint high-recall candidates for this P0 objective.
      const matches = await skills.embeddings.search(
        candidatesPrompt.search(input, node),
        routing.maxCandidates,
      );
      const candidates = canonicalCandidates(
        matches.map(({ data }) => data),
        catalog,
        routing.maxCandidates,
      );

      // Convert each complete skill body into short, goal-specific planning hints.
      const extracted = await Promise.all(
        candidates.map(async (skill) => {
          const { structured } = await provider.complete({
            messages: [
              { role: 'system', content: hintsPrompt.system() },
              {
                role: 'user',
                content: hintsPrompt.user(input, graph, node, skill),
              },
            ],
            model: models.default,
            schema: SkillHintExtractionSchema,
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
      if (skill === undefined || seen.has(skill.name)) return [];
      seen.add(skill.name);
      return [skill];
    })
    .slice(0, limit);
};
