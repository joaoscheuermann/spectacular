import { z } from 'zod';

export const SchemaVersion = z.literal(1);
export const Id = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/);
export const Hash = z.string().regex(/^sha256:[a-f0-9]{64}$/);
export const IsoDateTime = z.iso.datetime();
export const NonNegativeInteger = z.number().int().nonnegative().safe();
export const PositiveInteger = z.number().int().positive().safe();
export const Probability = z.number().finite().min(0).max(1);
export const JsonValue = z.json();

export const Domain = z.enum([
  'documents-finance',
  'software',
  'artifacts',
  'communication',
]);
export const CompositionClass = z.enum(['A', 'B', 'C', 'D', 'E', 'F']);
export const StudyPhase = z.enum([
  'calibration',
  'pilot',
  'confirmatory',
  'replication',
  'smoke',
]);

export const ModelConfig = z
  .object({
    provider: Id,
    model: z.string().trim().min(1),
    effort: z.enum(['none', 'minimal', 'low', 'medium', 'high', 'xhigh']),
  })
  .strict();

export const Usage = z
  .object({
    inputTokens: NonNegativeInteger,
    outputTokens: NonNegativeInteger,
    cachedInputTokens: NonNegativeInteger.optional(),
    reasoningTokens: NonNegativeInteger.optional(),
    embeddingInputTokens: NonNegativeInteger.optional(),
    rerankInputTokens: NonNegativeInteger.optional(),
    rerankDocuments: NonNegativeInteger.optional(),
    rerankSearchUnits: NonNegativeInteger.optional(),
    completionRequests: NonNegativeInteger.optional(),
    embeddingRequests: NonNegativeInteger.optional(),
    rerankRequests: NonNegativeInteger.optional(),
    modelCalls: NonNegativeInteger,
    toolCalls: NonNegativeInteger,
    costUsd: z.number().finite().nonnegative(),
  })
  .strict();

export type JsonValueV1 = z.infer<typeof JsonValue>;
