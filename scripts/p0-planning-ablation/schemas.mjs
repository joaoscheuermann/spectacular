import * as z from 'zod';

const nonEmptyString = z.string().refine((value) => value.trim().length > 0, {
  message: 'Value must contain non-whitespace characters.',
});
const sha256Schema = z.string().regex(/^[a-f0-9]{64}$/);
const uuidSchema = z.string().uuid();

const uniqueNames = (schema) =>
  schema.refine((names) => new Set(names).size === names.length, {
    message: 'Skill names must be unique.',
  });

const skillNames = uniqueNames(z.array(nonEmptyString));

const noiseSchema = z.record(
  nonEmptyString,
  z
    .object({
      type: z.enum(['irrelevant', 'no_operational_value', 'conflicting']),
    })
    .strict(),
);

export const caseSchema = z
  .object({
    name: nonEmptyString,
    objective: nonEmptyString,
    skills: z
      .object({
        expected: skillNames.min(1),
        useful: skillNames,
        noise: noiseSchema,
      })
      .superRefine(({ expected, useful, noise }, context) => {
        const owners = new Map();
        const categories = [
          ['expected', expected],
          ['useful', useful],
          ['noise', Object.keys(noise)],
        ];
        for (const [category, names] of categories) {
          for (const name of names) {
            const owner = owners.get(name);
            if (owner === undefined) {
              owners.set(name, category);
              continue;
            }
            context.addIssue({
              code: 'custom',
              message: `${name} appears in both ${owner} and ${category}.`,
              path: [category],
            });
          }
        }
      })
      .strict(),
  })
  .strict();

const frozenP0CaseSchema = z
  .object({
    name: nonEmptyString,
    objective: nonEmptyString,
    p0: z.array(nonEmptyString).min(1),
  })
  .strict();

export const fixtureSchema = z
  .object({
    schemaVersion: z.literal(1),
    source: z
      .object({
        runId: nonEmptyString,
        resultsSha256: z.string().regex(/^[a-f0-9]{64}$/),
      })
      .strict(),
    cases: z.array(frozenP0CaseSchema).length(30),
  })
  .strict();

export const goalsSchema = z
  .object({
    goals: z.array(nonEmptyString).min(1),
  })
  .strict();

export const judgmentChoices = Object.freeze(['a', 'b', 'both', 'neither']);

export const judgmentSchema = z
  .object({
    choice: z.enum(judgmentChoices),
    rationale: nonEmptyString,
  })
  .strict();

export const outcomeSchema = z.enum([
  'withoutP0',
  'withP0',
  'both',
  'neither',
  'inconsistent',
]);

const persistedJudgmentSchema = z
  .object({
    orientation: z.union([z.literal(1), z.literal(2)]),
    withoutP0Option: z.enum(['a', 'b']),
    choice: judgmentSchema.shape.choice,
    rationale: nonEmptyString,
    winner: outcomeSchema.exclude(['inconsistent']),
  })
  .strict();

const persistedCaseSchema = z
  .object({
    name: nonEmptyString,
    objective: nonEmptyString,
    p0: z.array(nonEmptyString).min(1),
    goldSkills: uniqueNames(z.array(nonEmptyString).min(1)),
    plans: z
      .object({
        withoutP0: z.array(nonEmptyString).min(1),
        withP0: z.array(nonEmptyString).min(1),
      })
      .strict(),
    judgments: z.array(persistedJudgmentSchema).length(2),
    rawOutcome: outcomeSchema,
    outcome: outcomeSchema,
    plansIdentical: z.boolean(),
  })
  .strict();

const hashIdentitySchema = z
  .object({ count: z.number().int().nonnegative(), sha256: sha256Schema })
  .strict();

const retrySchema = z
  .object({
    attempts: z.number().int().positive(),
    delayMs: z.number().int().nonnegative(),
    backoffMultiplier: z.number().int().positive(),
  })
  .strict();

export const generationManifestSchema = z
  .object({
    schemaVersion: z.literal(1),
    runId: uuidSchema,
    mode: z.literal('generation').optional(),
    status: z.literal('completed'),
    startedAt: nonEmptyString,
    completedAt: nonEmptyString,
    config: z
      .object({
        planningModel: nonEmptyString,
        planningEffort: nonEmptyString,
        judgeModel: nonEmptyString,
        judgeEffort: nonEmptyString,
        comparisonProtocolSha256: sha256Schema.optional(),
        retry: retrySchema,
      })
      .strict(),
    identity: z
      .object({
        repository: z
          .object({ commit: nonEmptyString, dirty: z.boolean() })
          .strict(),
        fixture: z
          .object({
            cases: z.literal(30),
            runId: uuidSchema,
            sourceResultsSha256: sha256Schema,
            sha256: sha256Schema,
          })
          .strict(),
        cases: hashIdentitySchema,
        catalog: hashIdentitySchema,
        experimentSources: hashIdentitySchema,
        packageMetadata: hashIdentitySchema,
        environment: z
          .object({
            node: nonEmptyString,
            platform: nonEmptyString,
            arch: nonEmptyString,
          })
          .strict(),
      })
      .strict(),
    files: z
      .object({
        log: z.literal('output.log'),
        results: z.literal('results.json'),
      })
      .strict(),
    cases: z.literal(30),
    metrics: z.unknown(),
    providerUsage: z.unknown(),
  })
  .strict();

export const generationResultsSchema = z
  .object({
    runId: uuidSchema,
    mode: z.literal('generation').optional(),
    cases: z.literal(30),
    metrics: z.unknown(),
    providerUsage: z.unknown(),
    results: z.array(persistedCaseSchema).length(30),
  })
  .strict();

export const rejudgeCampaignSchema = z
  .object({
    schemaVersion: z.literal(1),
    comparisonProtocolSha256: sha256Schema,
    sources: z
      .array(
        z
          .object({
            runId: uuidSchema,
            manifestSha256: sha256Schema,
            resultsSha256: sha256Schema,
            sourceExperimentSourcesSha256: sha256Schema,
            attestedComparisonProtocolSha256: sha256Schema,
            sourceJudgeModel: nonEmptyString,
            targetJudgeModel: nonEmptyString,
            targetJudgeEffort: nonEmptyString,
          })
          .strict(),
      )
      .min(1),
  })
  .strict();
