import { randomUUID } from 'node:crypto';

import { ModelRole, type Prisma } from '../generated/prisma/client.js';
import {
  ConfigInputSchema,
  type ConfigInput,
  type DoricConfig,
} from './config.js';
import type { Database } from './database.js';

const singletonId = 1;

type StoredConfig = Prisma.DoricConfigurationGetPayload<{
  include: { providers: true; models: true };
}>;

export type ConfigStore = {
  load(): Promise<DoricConfig>;
  replace(config: ConfigInput): Promise<DoricConfig>;
};

/** Persists the singleton configuration and its normalized provider/model rows. */
export const createConfigStore = (database: Database): ConfigStore => ({
  async load() {
    const stored = await database.doricConfiguration.findUniqueOrThrow({
      where: { id: singletonId },
      include: { providers: true, models: true },
    });
    return fromStored(stored);
  },

  async replace(config) {
    return database.$transaction(
      async (transaction) => {
        await transaction.modelConfiguration.deleteMany({
          where: { configurationId: singletonId },
        });
        await transaction.providerConfiguration.deleteMany({
          where: { configurationId: singletonId },
        });
        await transaction.providerConfiguration.createMany({
          data: config.providers.map((provider) => ({
            configurationId: singletonId,
            ...provider,
          })),
        });
        await transaction.modelConfiguration.createMany({
          data: modelRows(config),
        });
        const stored = await transaction.doricConfiguration.update({
          where: { id: singletonId },
          data: {
            revision: { increment: 1 },
            generation: randomUUID(),
            maxHintCandidates: config.routing.maxHintCandidates,
            maxRetrievedCandidates: config.routing.maxRetrievedCandidates,
            maxSkills: config.routing.maxSkills,
            maxTurns: config.execution.maxTurns,
            maxRevisions: config.revision.max,
          },
          include: { providers: true, models: true },
        });
        return fromStored(stored);
      },
      { isolationLevel: 'Serializable' },
    );
  },
});

const modelRows = (config: ConfigInput) => [
  modelRow(ModelRole.PLANNING, config.models.planning),
  modelRow(ModelRole.REVISION, config.models.revision),
  modelRow(ModelRole.EXECUTION, config.models.execution),
  modelRow(ModelRole.RERANKER, config.models.reranker),
  modelRow(ModelRole.EMBEDDER, config.models.embedder),
];

const modelRow = (
  role: ModelRole,
  profile: ConfigInput['models'][keyof ConfigInput['models']],
) => ({
  configurationId: singletonId,
  role,
  providerId: profile.providerId,
  model: profile.model,
  effort: 'effort' in profile ? profile.effort : null,
  dimensions: 'dimensions' in profile ? profile.dimensions : null,
});

const fromStored = (stored: StoredConfig): DoricConfig => {
  const models = new Map(stored.models.map((model) => [model.role, model]));
  const reasoning = (role: ModelRole) => {
    const profile = required(models, role);
    if (profile.effort === null)
      throw new Error('Stored reasoning effort is missing.');
    return {
      providerId: profile.providerId,
      model: profile.model,
      effort: profile.effort as ConfigInput['models']['planning']['effort'],
    };
  };
  const plain = (role: ModelRole) => {
    const profile = required(models, role);
    return { providerId: profile.providerId, model: profile.model };
  };
  const embedding = required(models, ModelRole.EMBEDDER);
  if (embedding.dimensions === null)
    throw new Error('Stored embedding dimensions are missing.');

  const configuration = ConfigInputSchema.parse({
    providers: stored.providers
      .map(({ id, baseUrl, apiKeyEnv }) => ({ id, baseUrl, apiKeyEnv }))
      .sort((left, right) => left.id.localeCompare(right.id)),
    models: {
      planning: reasoning(ModelRole.PLANNING),
      revision: reasoning(ModelRole.REVISION),
      execution: reasoning(ModelRole.EXECUTION),
      reranker: plain(ModelRole.RERANKER),
      embedder: {
        ...plain(ModelRole.EMBEDDER),
        dimensions: embedding.dimensions,
      },
    },
    routing: {
      maxHintCandidates: stored.maxHintCandidates,
      maxRetrievedCandidates: stored.maxRetrievedCandidates,
      maxSkills: stored.maxSkills,
    },
    execution: { maxTurns: stored.maxTurns },
    revision: { max: stored.maxRevisions },
  });

  return {
    configuration,
    revision: stored.revision,
    updatedAt: stored.updatedAt.toISOString(),
  };
};

const required = <Value>(
  values: ReadonlyMap<ModelRole, Value>,
  role: ModelRole,
): Value => {
  const value = values.get(role);
  if (value === undefined)
    throw new Error(`Stored Mosaic model role is missing: ${role}`);
  return value;
};
