import * as z from 'zod';

const idSchema = z.string().regex(/^[a-z0-9][a-z0-9-]*$/u);
const folderIdSchema = z
  .string()
  .regex(/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/u)
  .refine((id) => !/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/u.test(id), {
    message: 'model id is a reserved Windows device name',
  });
const textSchema = z.string().trim().min(1);
const effortSchema = z.enum([
  'none',
  'minimal',
  'low',
  'medium',
  'high',
  'xhigh',
]);

export const providerTypeSchema = z.enum([
  'openai',
  'openrouter',
  'lmstudio',
  'lmstudio-openai',
  'codex',
]);

export const providerConfigSchema = z
  .object({
    id: idSchema,
    type: providerTypeSchema,
    baseUrl: z.string().url().optional(),
    tokenEnv: z
      .string()
      .regex(/^[A-Z_][A-Z0-9_]*$/u)
      .optional(),
  })
  .strict();

export const modelRefSchema = z
  .object({
    provider: idSchema,
    model: textSchema,
    effort: effortSchema.optional(),
    temperature: z.number().finite().min(0).optional(),
    maxOutputTokens: z.number().int().positive().optional(),
  })
  .strict();

export const targetModelSchema = modelRefSchema.extend({
  id: folderIdSchema.refine((id) => id !== 'default' && id !== 'scenarios', {
    message: 'model id is reserved',
  }),
});

export const evolutionConfigSchema = z
  .object({
    providers: z.array(providerConfigSchema).min(1),
    models: z.array(targetModelSchema).min(1),
    optimizer: modelRefSchema,
    judges: z.array(modelRefSchema).min(2),
    evolution: z
      .object({
        targetAccuracy: z.number().finite().min(0).max(1),
        plateauPatience: z.number().int().positive(),
        maxEpochs: z.number().int().positive(),
      })
      .strict(),
  })
  .strict();

export const scenarioSchema = z
  .object({
    id: idSchema,
    input: textSchema,
    expected: textSchema,
    rationale: textSchema.optional(),
    tags: z.array(textSchema).default([]),
  })
  .strict();

export const judgmentSchema = z
  .object({
    passed: z.boolean(),
    ambiguous: z.boolean(),
    rationale: textSchema,
  })
  .strict();

export const proposalOutputSchema = z
  .object({
    prompt: textSchema,
    scenarios: z.array(
      z
        .object({
          id: idSchema,
          input: textSchema,
          expected: textSchema,
          rationale: textSchema.nullable(),
          tags: z.array(textSchema),
        })
        .strict(),
    ),
    rationale: textSchema,
  })
  .strict();

export type ProviderConfig = z.infer<typeof providerConfigSchema>;
export type ProviderType = z.infer<typeof providerTypeSchema>;
export type ModelRef = z.infer<typeof modelRefSchema>;
export type TargetModel = z.infer<typeof targetModelSchema>;
export type EvolutionConfig = z.infer<typeof evolutionConfigSchema>;
export type Scenario = z.infer<typeof scenarioSchema>;
export type Judgment = z.infer<typeof judgmentSchema>;
export type ProposalOutput = z.infer<typeof proposalOutputSchema>;
