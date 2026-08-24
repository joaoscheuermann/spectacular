import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';

import { z } from 'zod';

import {
  aggregateSraRetrievalMetrics,
  type SraAggregateRetrievalMetrics,
  type SraRetrievalRecord,
} from './sra-metrics.js';

export const sraSkillProjections = ['selected', 'candidates'] as const;
export type SraSkillProjection = (typeof sraSkillProjections)[number];

const IdentifierSchema = z.string().trim().min(1);
const GoldSchema = z
  .object({
    instance_id: IdentifierSchema,
    dataset: IdentifierSchema,
    gold_skill_ids: z.array(IdentifierSchema).min(1),
  })
  .strict();
const InferenceSchema = z
  .object({
    instance_id: IdentifierSchema,
    dataset: IdentifierSchema,
    skill_ids_used: z.array(IdentifierSchema).optional(),
    meta: z
      .object({ candidate_skill_ids: z.array(IdentifierSchema) })
      .passthrough(),
  })
  .passthrough();

export interface SraOutputSkillMetrics {
  readonly projection: SraSkillProjection;
  readonly dataset: string;
  readonly metrics: SraAggregateRetrievalMetrics;
}

type Gold = z.output<typeof GoldSchema>;
type Inference = z.output<typeof InferenceSchema>;

/** Parses the frozen pilot gold artifact emitted by `sra prepare`. */
export const parseSraSkillGoldsJson = (source: string): readonly Gold[] => {
  let value: unknown;
  try {
    value = JSON.parse(source) as unknown;
  } catch {
    throw new Error('SRA skill gold artifact is not valid JSON.');
  }
  const records = z.array(GoldSchema).min(1).parse(value);
  assertUnique(
    records.map(({ instance_id: id }) => id),
    'gold instance',
  );
  records.forEach((record) =>
    assertUnique(record.gold_skill_ids, `gold skill for ${record.instance_id}`),
  );
  return records;
};

/** Parses one append-only MOSAIC SRA inference JSONL artifact. */
export const parseSraInferenceJsonl = (
  source: string,
): readonly Inference[] => {
  const lines = source.split('\n').filter((line) => line.trim() !== '');
  if (lines.length === 0) throw new Error('SRA inference output is empty.');
  const records = lines.map((line, index) => {
    try {
      return InferenceSchema.parse(JSON.parse(line) as unknown);
    } catch {
      throw new Error(`Invalid SRA inference record at line ${index + 1}.`);
    }
  });
  assertUnique(
    records.map(({ instance_id: id }) => id),
    'inference instance',
  );
  return records;
};

/** Scores the selected bundle or the routed candidate list against pilot golds. */
export const scoreSraOutputSkills = (
  inference: readonly Inference[],
  golds: readonly Gold[],
  projection: SraSkillProjection,
  k: number,
): SraOutputSkillMetrics => {
  if (!sraSkillProjections.includes(projection))
    throw new Error(`Unknown SRA skill projection: ${projection as string}`);
  const datasets = new Set(inference.map(({ dataset }) => dataset));
  if (datasets.size !== 1)
    throw new Error('SRA output scoring requires one dataset.');
  const dataset = inference[0]!.dataset;
  const goldById = new Map(
    golds
      .filter((record) => record.dataset === dataset)
      .map((record) => [record.instance_id, record]),
  );
  assertSameInstances(inference, goldById);
  const records: readonly SraRetrievalRecord[] = inference.map((record) => {
    const gold = goldById.get(record.instance_id)!;
    if (gold.dataset !== record.dataset)
      throw new Error(`SRA dataset differs for ${record.instance_id}.`);
    const ids =
      projection === 'selected'
        ? (record.skill_ids_used ?? [])
        : record.meta.candidate_skill_ids;
    assertUnique(ids, `${projection} skill for ${record.instance_id}`);
    return {
      instance_id: record.instance_id,
      gold_skill_ids: gold.gold_skill_ids,
      retrieved: ids.map((skill_id) => ({ skill_id })),
    };
  });
  return {
    projection,
    dataset,
    metrics: aggregateSraRetrievalMetrics(records, k),
  };
};

/** Scores persisted inference and gold artifacts and optionally writes JSON. */
export const scoreSraOutputFile = async (
  inputPath: string,
  goldPath: string,
  projection: SraSkillProjection,
  k: number,
  outputPath?: string,
): Promise<SraOutputSkillMetrics> => {
  const [input, gold] = await Promise.all([
    readFile(inputPath, 'utf8'),
    readFile(goldPath, 'utf8'),
  ]);
  const result = scoreSraOutputSkills(
    parseSraInferenceJsonl(input),
    parseSraSkillGoldsJson(gold),
    projection,
    k,
  );
  if (outputPath !== undefined) {
    await mkdir(dirname(outputPath), { recursive: true });
    await writeFile(outputPath, `${JSON.stringify(result, null, 2)}\n`, 'utf8');
  }
  return result;
};

const assertSameInstances = (
  inference: readonly Inference[],
  goldById: ReadonlyMap<string, Gold>,
): void => {
  if (
    inference.length !== goldById.size ||
    inference.some(({ instance_id: id }) => !goldById.has(id))
  ) {
    throw new Error('SRA inference and gold instance sets differ.');
  }
};

const assertUnique = (values: readonly string[], label: string): void => {
  if (new Set(values).size !== values.length)
    throw new Error(`Duplicate ${label} identifier.`);
};
