import type { Bundle, Skill } from 'bundle';
import {
  createFetchTransport,
  createOpenAiCompatibleProvider,
  type LlmProvider,
} from 'llms';
import type { Logger } from 'pino';
import type { ToolFactory } from 'tool';
import {
  createHybridSearch,
  createLexicalIndex,
  createVectorIndex,
  type Search,
} from 'victor';

import type { ConfigInput, DoricConfig } from './config.js';

export type Catalog = {
  readonly skills: readonly {
    readonly skill: Skill;
    readonly alwaysAvailable: boolean;
  }[];
  readonly tools: readonly {
    readonly factory: ToolFactory;
    readonly alwaysAvailable: boolean;
  }[];
};

type Retrieval = {
  readonly fingerprint: string;
  readonly skills: Search<Skill>;
  readonly tools: Search<ToolFactory>;
};

export type Generation = {
  readonly snapshot: DoricConfig;
  readonly providers: ReadonlyMap<string, LlmProvider>;
  readonly retrieval: Retrieval;
  readonly catalog: Catalog;
};

type GenerationOptions = {
  readonly snapshot: DoricConfig;
  readonly bundles: readonly Bundle[];
  readonly logger: Logger;
  readonly environment?: NodeJS.ProcessEnv;
  readonly previous?: Generation;
  readonly signal?: AbortSignal;
};

/** Builds a credential-resolving provider generation and, when needed, new indexes. */
export const createGeneration = async ({
  snapshot,
  bundles,
  logger,
  environment = process.env,
  previous,
  signal,
}: GenerationOptions): Promise<Generation> => {
  const config = snapshot.configuration;
  const providers = new Map(
    config.providers.map((provider) => [
      provider.id,
      createOpenAiCompatibleProvider({
        transport: createFetchTransport(),
        baseUrl: provider.baseUrl,
        apiKey: () => environment[provider.apiKeyEnv] ?? '',
        identity: { id: provider.id, name: provider.id },
        logger,
      }),
    ]),
  );
  const catalog = catalogs(bundles);
  const fingerprint = retrievalFingerprint(config);
  const retrieval =
    previous?.retrieval.fingerprint === fingerprint
      ? previous.retrieval
      : await createRetrieval({
          config,
          providers,
          catalog,
          fingerprint,
          logger,
          signal,
        });

  return { snapshot, providers, retrieval, catalog };
};

export const providerFor = (
  generation: Generation,
  id: string,
): LlmProvider => {
  const provider = generation.providers.get(id);
  if (provider === undefined)
    throw new Error('Configured provider is unavailable.');
  return provider;
};

const catalogs = (bundles: readonly Bundle[]): Catalog => ({
  skills: bundles.flatMap(({ skills }) => skills),
  tools: bundles.flatMap(({ tools }) => tools),
});

const retrievalFingerprint = (config: ConfigInput): string => {
  const model = config.models.embedder;
  const provider = config.providers.find(({ id }) => id === model.providerId);
  if (provider === undefined)
    throw new Error('Embedder provider is unavailable.');
  return JSON.stringify({ provider, model });
};

type RetrievalOptions = {
  readonly config: ConfigInput;
  readonly providers: ReadonlyMap<string, LlmProvider>;
  readonly catalog: Catalog;
  readonly fingerprint: string;
  readonly logger: Logger;
  readonly signal?: AbortSignal;
};

const createRetrieval = async ({
  config,
  providers,
  catalog,
  fingerprint,
  logger,
  signal,
}: RetrievalOptions): Promise<Retrieval> => {
  const profile = config.models.embedder;
  const provider = providers.get(profile.providerId);
  if (provider === undefined)
    throw new Error('Embedder provider is unavailable.');
  const embed = (input: string) =>
    provider.embedding({
      model: profile.model,
      input,
      dimensions: profile.dimensions,
      signal,
      flags: { sensitiveOutput: true },
    });
  const skillLexical = createLexicalIndex<Skill>({ logger });
  const skillVector = createVectorIndex<Skill>({
    dimensions: profile.dimensions,
    embedding: embed,
    logger,
  });
  const toolLexical = createLexicalIndex<ToolFactory>({ logger });
  const toolVector = createVectorIndex<ToolFactory>({
    dimensions: profile.dimensions,
    embedding: embed,
    logger,
  });
  const required = new Set(
    catalog.skills
      .filter(({ alwaysAvailable }) => alwaysAvailable)
      .map(({ skill }) => skill.name),
  );

  for (const { skill } of catalog.skills) {
    if (required.has(skill.name)) continue;
    signal?.throwIfAborted();
    await Promise.all([
      skillLexical.add(skill, ({ indexText }) => indexText),
      skillVector.add(skill, ({ indexText }) => indexText),
    ]);
  }
  for (const { factory } of catalog.tools) {
    signal?.throwIfAborted();
    const text = ({ name, description }: ToolFactory) =>
      `${name} | ${description ?? ''}`;
    await Promise.all([
      toolLexical.add(factory, text),
      toolVector.add(factory, text),
    ]);
  }

  return {
    fingerprint,
    skills: createHybridSearch({
      lexical: skillLexical,
      semantic: skillVector,
      key: ({ name }) => name,
      logger,
    }),
    tools: createHybridSearch({
      lexical: toolLexical,
      semantic: toolVector,
      key: ({ name }) => name,
      logger,
    }),
  };
};
