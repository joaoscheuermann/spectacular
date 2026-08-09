import { z } from 'zod';

const effort = z.enum(['none', 'minimal', 'low', 'medium', 'high', 'xhigh']);
const identifier = z.string().trim().min(1).max(128);
const model = z.string().trim().min(1).max(512);
const limit = z.number().int().safe().positive();

const provider = z
  .object({
    id: identifier,
    baseUrl: z
      .url()
      .refine(
        (value) => value.startsWith('http://') || value.startsWith('https://'),
        {
          message: 'Provider baseUrl must use HTTP or HTTPS.',
        },
      ),
    apiKeyEnv: z
      .string()
      .regex(
        /^[A-Z][A-Z0-9_]*_API_KEY$/u,
        'Invalid provider API key environment name.',
      ),
  })
  .strict();

const reasoningModel = z
  .object({
    providerId: identifier,
    model,
    effort,
  })
  .strict();

const plainModel = z
  .object({
    providerId: identifier,
    model,
  })
  .strict();

const embedderModel = plainModel.extend({ dimensions: limit }).strict();

/** Complete, credential-free Mosaic configuration accepted by the REST API. */
export const ConfigInputSchema = z
  .object({
    providers: z.array(provider).min(1),
    models: z
      .object({
        planning: reasoningModel,
        revision: reasoningModel,
        execution: reasoningModel,
        reranker: plainModel,
        embedder: embedderModel,
      })
      .strict(),
    routing: z
      .object({
        maxHintCandidates: limit,
        maxRetrievedCandidates: limit,
        maxSkills: z.number().int().safe().nonnegative(),
      })
      .strict(),
    execution: z.object({ maxTurns: limit }).strict(),
    revision: z.object({ max: z.number().int().safe().nonnegative() }).strict(),
  })
  .strict()
  .superRefine((value, context) => {
    const ids = new Set<string>();
    value.providers.forEach(({ id }, index) => {
      if (ids.has(id)) {
        context.addIssue({
          code: 'custom',
          message: 'Provider IDs must be unique.',
          path: ['providers', index, 'id'],
        });
      }
      ids.add(id);
    });

    Object.entries(value.models).forEach(([role, profile]) => {
      if (!ids.has(profile.providerId)) {
        context.addIssue({
          code: 'custom',
          message: 'Model references an unavailable provider.',
          path: ['models', role, 'providerId'],
        });
      }
    });

    if (value.routing.maxSkills > value.routing.maxRetrievedCandidates) {
      context.addIssue({
        code: 'custom',
        message: 'maxSkills must not exceed maxRetrievedCandidates.',
        path: ['routing', 'maxSkills'],
      });
    }
  });

export type ConfigInput = z.output<typeof ConfigInputSchema>;

export type DoricConfig = {
  readonly configuration: ConfigInput;
  readonly revision: number;
  readonly updatedAt: string;
};

export const defaultConfig: ConfigInput = {
  providers: [
    {
      id: 'openrouter',
      baseUrl: 'https://openrouter.ai/api/v1',
      apiKeyEnv: 'OPENROUTER_API_KEY',
    },
  ],
  models: {
    planning: {
      providerId: 'openrouter',
      model: 'qwen/qwen3.7-flash',
      effort: 'low',
    },
    revision: {
      providerId: 'openrouter',
      model: 'google/gemini-3.6-flash',
      effort: 'low',
    },
    execution: {
      providerId: 'openrouter',
      model: 'deepseek/deepseek-v4-flash-0731',
      effort: 'low',
    },
    reranker: {
      providerId: 'openrouter',
      model: 'voyageai/rerank-2.5-lite',
    },
    embedder: {
      providerId: 'openrouter',
      model: 'voyageai/voyage-4-large',
      dimensions: 2048,
    },
  },
  routing: {
    maxHintCandidates: 5,
    maxRetrievedCandidates: 5,
    maxSkills: 5,
  },
  execution: { maxTurns: 32 },
  revision: { max: 3 },
};
