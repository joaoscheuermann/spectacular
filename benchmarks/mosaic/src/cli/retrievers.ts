import type { LlmProvider } from 'llms';
import type { Logger } from 'pino';
import {
  createHybridSearch,
  createLexicalIndex,
  createVectorIndex,
  type Search,
} from 'victor';

import { mosaicSkills, type MosaicDependencies } from '../conditions/index.js';
import { EMBEDDING_MODEL } from '../config/index.js';
import { progress } from './io.js';

type Skill = ReturnType<typeof mosaicSkills>[number];

const metadataText = (skill: Skill): string =>
  `${skill.name} | ${skill.description} | ${skill.allowedTools.join(',')}`;

const build = async (
  values: readonly Skill[],
  text: (skill: Skill) => string,
  provider: LlmProvider,
  logger: Logger,
): Promise<Search<Skill>> => {
  const lexical = createLexicalIndex<Skill>({ logger });
  const semantic = createVectorIndex<Skill>({
    dimensions: EMBEDDING_MODEL.dimensions,
    logger,
    embedding: (input) =>
      provider.embedding({
        model: EMBEDDING_MODEL.model,
        input,
        dimensions: EMBEDDING_MODEL.dimensions,
        flags: { sensitiveOutput: true },
      }),
  });
  for (const skill of values) {
    await Promise.all([lexical.add(skill, text), semantic.add(skill, text)]);
  }
  return createHybridSearch({
    lexical,
    semantic,
    key: (skill) => skill.name,
    logger,
  });
};

/** Builds the body and metadata views with the same frozen hybrid algorithm. */
export const buildProductionRetrievers = async (
  provider: LlmProvider,
  logger: Logger,
): Promise<MosaicDependencies['retrievers']> => {
  const skills = mosaicSkills();
  progress('indexing frozen skill body view');
  const body = await build(
    skills,
    (skill) => skill.indexText,
    provider,
    logger,
  );
  progress('indexing frozen skill metadata view');
  const metadata = await build(skills, metadataText, provider, logger);

  return {
    skills: body,
    metadataSkills: metadata,
    // MOSAIC derives executable menus from selected skill declarations. The
    // tool retriever remains a required public option but is not queried.
    tools: { search: async () => [] },
  };
};
