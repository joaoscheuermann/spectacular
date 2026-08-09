import type { LlmProvider } from 'llms';
import type { Logger } from 'pino';
import {
  createHybridSearch,
  createLexicalIndex,
  createVectorIndex,
  type Search,
} from 'victor';

import { mosaicSkills, type MosaicDependencies } from '../conditions/index.js';
import type { IndexArtifact } from './index-lifecycle.js';

type Skill = ReturnType<typeof mosaicSkills>[number];
type View = IndexArtifact['entries'][number]['view'];

const metadataText = (skill: Skill): string =>
  `${skill.name} | ${skill.description} | ${skill.allowedTools.join(',')}`;

const build = async (
  values: readonly Skill[],
  text: (skill: Skill) => string,
  view: View,
  artifact: IndexArtifact,
  provider: LlmProvider,
  logger: Logger,
): Promise<Search<Skill>> => {
  const vectors = new Map(
    artifact.entries
      .filter((entry) => entry.view === view)
      .map((entry) => [entry.skillName, entry.vector]),
  );
  let building = true;
  let currentSkill: Skill | undefined;
  const lexical = createLexicalIndex<Skill>({ logger });
  const semantic = createVectorIndex<Skill>({
    dimensions: artifact.recipe.dimensions,
    logger,
    embedding: async (input) => {
      if (building && currentSkill !== undefined) {
        const vector = vectors.get(currentSkill.name);
        if (vector === undefined || input !== text(currentSkill)) {
          throw new Error('retrieval index does not match the indexed view');
        }
        return vector;
      }
      return provider.embedding({
        model: artifact.recipe.embedder,
        input,
        dimensions: artifact.recipe.dimensions,
        flags: { sensitiveOutput: true },
      });
    },
  });
  for (const skill of values) {
    currentSkill = skill;
    await Promise.all([lexical.add(skill, text), semantic.add(skill, text)]);
  }
  currentSkill = undefined;
  building = false;
  return createHybridSearch({
    lexical,
    semantic,
    key: (skill) => skill.name,
    logger,
  });
};

/** Rehydrates both views without paid calls; only later queries are metered. */
export const buildProductionRetrievers = async (
  provider: LlmProvider,
  logger: Logger,
  artifact: IndexArtifact,
): Promise<MosaicDependencies['retrievers']> => {
  const skills = mosaicSkills();
  const [body, metadata] = await Promise.all([
    build(
      skills,
      (skill) => skill.indexText,
      'body',
      artifact,
      provider,
      logger,
    ),
    build(skills, metadataText, 'metadata', artifact, provider, logger),
  ]);
  return {
    skills: body,
    metadataSkills: metadata,
    // MOSAIC derives executable menus from selected skill declarations. The
    // tool retriever remains a required public option but is not queried.
    tools: { search: async () => [] },
  };
};
