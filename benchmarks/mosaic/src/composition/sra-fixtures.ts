import { z } from 'zod';

const identifierSchema = z
  .string()
  .min(1)
  .refine((value) => value === value.trim(), 'identifier must be trimmed');

const findDuplicate = (values: readonly string[]): string | undefined => {
  const seen = new Set<string>();
  for (const value of values) {
    if (seen.has(value)) return value;
    seen.add(value);
  }
  return undefined;
};

const skillAnnotationsSchema = z
  .array(identifierSchema)
  .min(1)
  .superRefine((skillIds, context) => {
    const duplicate = findDuplicate(skillIds);
    if (duplicate) {
      context.addIssue({
        code: 'custom',
        message: `duplicate skill annotation: ${duplicate}`,
      });
    }
  });

const corpusSkillSchema = z
  .object({
    skill_id: identifierSchema,
    name: z.string(),
    description: z.string(),
    content: z.string(),
  })
  .passthrough();

const instanceSchema = z
  .object({
    instance_id: identifierSchema,
    dataset: identifierSchema,
    question: z.string().min(1),
    skill_annotations: skillAnnotationsSchema,
  })
  .passthrough();

const corpusSchema = z.array(corpusSkillSchema);
const instancesSchema = z.array(instanceSchema);

export type SraCorpusSkill = {
  readonly [key: string]: unknown;
  readonly skill_id: string;
  readonly name: string;
  readonly description: string;
  readonly content: string;
};

export type SraInstance = {
  readonly [key: string]: unknown;
  readonly instance_id: string;
  readonly dataset: string;
  readonly question: string;
  readonly skill_annotations: readonly string[];
};

const assertUniqueField = <T>(
  records: readonly T[],
  field: (record: T) => string,
  label: string,
): void => {
  const duplicate = findDuplicate(records.map(field));
  if (duplicate) throw new Error(`duplicate ${label}: ${duplicate}`);
};

const parseJson = (source: string, label: string): unknown => {
  try {
    return JSON.parse(source) as unknown;
  } catch (cause) {
    throw new Error(`SRA-Bench ${label} fixture is not valid JSON.`, {
      cause,
    });
  }
};

/** Validates an already parsed SRA-Bench corpus while retaining extra fields. */
export const parseSraCorpus = (input: unknown): readonly SraCorpusSkill[] => {
  const corpus = corpusSchema.parse(input);
  assertUniqueField(corpus, (skill) => skill.skill_id, 'skill_id');
  return corpus;
};

/** Parses and validates the contents of a locally downloaded corpus JSON file. */
export const parseSraCorpusJson = (source: string): readonly SraCorpusSkill[] =>
  parseSraCorpus(parseJson(source, 'corpus'));

/** Validates already parsed SRA-Bench instances while retaining extra fields. */
export const parseSraInstances = (input: unknown): readonly SraInstance[] => {
  const instances = instancesSchema.parse(input);
  assertUniqueField(
    instances,
    (instance) => instance.instance_id,
    'instance_id',
  );
  return instances;
};

/** Parses and validates the contents of a locally downloaded instances file. */
export const parseSraInstancesJson = (source: string): readonly SraInstance[] =>
  parseSraInstances(parseJson(source, 'instances'));
