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

const optimizerRefSchema = z
  .object({
    provider: idSchema,
    model: textSchema,
    effort: effortSchema.optional(),
    maxOutputTokens: z.number().int().positive().optional(),
  })
  .strict();

export const targetModelSchema = modelRefSchema.extend({
  id: folderIdSchema.refine((id) => id !== 'default' && id !== 'scenarios', {
    message: 'model id is reserved',
  }),
});

export const evalAssertionSchema = z
  .object({
    id: idSchema,
    assertion: textSchema,
  })
  .strict();

export const evolutionConfigSchema = z
  .object({
    providers: z.array(providerConfigSchema).min(1),
    models: z.array(targetModelSchema).min(1),
    optimizer: optimizerRefSchema,
    judge: modelRefSchema,
    evals: z.array(evalAssertionSchema),
    evolution: z
      .object({
        accuracy: z.number().finite().min(0).max(1),
        patience: z.object({ epochs: z.number().int().positive() }).strict(),
        epochs: z.number().int().positive(),
        history: z
          .object({ limit: z.number().int().positive().default(20) })
          .strict()
          .default({ limit: 20 }),
      })
      .strict(),
  })
  .strict();

export const scenarioSchema = z
  .object({
    id: idSchema,
    split: z.enum(['train', 'validation']),
    input: textSchema,
    evals: z.array(evalAssertionSchema),
    rationale: textSchema.optional(),
  })
  .strict();

export const evalVerdictSchema = z
  .object({
    evalId: idSchema,
    sampleIndex: z.number().int().min(0).max(2),
    reasoning: textSchema,
    passed: z.boolean(),
  })
  .strict();

export const judgeOutputSchema = z
  .object({
    results: z.array(evalVerdictSchema),
  })
  .strict();

export const proposalOutputSchema = z
  .object({
    prompt: textSchema,
    strategy: textSchema,
  })
  .strict();

export type ProviderConfig = z.infer<typeof providerConfigSchema>;
export type ProviderType = z.infer<typeof providerTypeSchema>;
export type ModelRef = z.infer<typeof modelRefSchema>;
export type TargetModel = z.infer<typeof targetModelSchema>;
export type EvalAssertion = z.infer<typeof evalAssertionSchema>;
export type EvolutionConfig = z.infer<typeof evolutionConfigSchema>;
export type Scenario = z.infer<typeof scenarioSchema>;
export type EvalVerdict = z.infer<typeof evalVerdictSchema>;
export type ProposalOutput = z.infer<typeof proposalOutputSchema>;
