export type SraRankedSkill = {
  readonly skill_id: string;
  readonly score?: number;
};

export type SraRetrievalRecord = {
  readonly instance_id: string;
  readonly gold_skill_ids: readonly string[];
  readonly retrieved: readonly SraRankedSkill[];
};

export type SraQueryRetrievalMetrics = {
  readonly instanceId: string;
  readonly k: number;
  readonly hits: number;
  readonly recallAtK: number;
  readonly setPrecisionAtK: number;
  readonly setF1AtK: number;
  readonly reciprocalRank: number;
  readonly ndcgAtK: number;
  readonly exactMatch: boolean;
  readonly goldCardinality: number;
  readonly retrievedCardinality: number;
  readonly cardinalityMatch: boolean;
  readonly cardinalityAbsoluteError: number;
};

export type SraAggregateRetrievalMetrics = {
  readonly k: number;
  readonly queryCount: number;
  readonly recallAtK: number;
  readonly setPrecisionAtK: number;
  readonly setF1AtK: number;
  readonly mrr: number;
  readonly ndcgAtK: number;
  readonly exactMatchRate: number;
  readonly cardinalityAccuracy: number;
  readonly meanAbsoluteCardinalityError: number;
  readonly perQuery: readonly SraQueryRetrievalMetrics[];
};

const assertPositiveInteger = (value: number, label: string): void => {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${label} must be a positive integer.`);
  }
};

const assertUniqueSkillIds = (
  skillIds: readonly string[],
  label: 'gold' | 'retrieved',
): void => {
  const seen = new Set<string>();

  for (const skillId of skillIds) {
    if (!skillId || skillId !== skillId.trim()) {
      throw new Error(`${label} skill_id must be a non-empty trimmed string.`);
    }

    if (seen.has(skillId)) {
      throw new Error(`duplicate ${label} skill_id: ${skillId}`);
    }

    seen.add(skillId);
  }
};

const discountedGain = (
  retrievedSkillIds: readonly string[],
  goldSkillIds: ReadonlySet<string>,
): number =>
  retrievedSkillIds.reduce(
    (total, skillId, index) =>
      total + (goldSkillIds.has(skillId) ? 1 / Math.log2(index + 2) : 0),
    0,
  );

const idealDiscountedGain = (goldCount: number, k: number): number =>
  Array.from({ length: Math.min(goldCount, k) }, (_, index) => index).reduce(
    (total, index) => total + 1 / Math.log2(index + 2),
    0,
  );

/** Computes pure binary-relevance retrieval and set metrics for one query. */
export const scoreSraRetrieval = (
  record: SraRetrievalRecord,
  k: number,
): SraQueryRetrievalMetrics => {
  assertPositiveInteger(k, 'k');

  if (!record.instance_id || record.instance_id !== record.instance_id.trim()) {
    throw new Error('instance_id must be a non-empty trimmed string.');
  }

  if (record.gold_skill_ids.length === 0) {
    throw new Error('gold_skill_ids must contain at least one skill.');
  }

  const retrievedSkillIds = record.retrieved.map((skill) => skill.skill_id);

  assertUniqueSkillIds(record.gold_skill_ids, 'gold');

  assertUniqueSkillIds(retrievedSkillIds, 'retrieved');

  const gold = new Set(record.gold_skill_ids);
  const selected = retrievedSkillIds.slice(0, k);
  const hits = selected.filter((skillId) => gold.has(skillId)).length;
  const recallAtK = hits / gold.size;
  const setPrecisionAtK = selected.length === 0 ? 0 : hits / selected.length;

  const setF1AtK =
    recallAtK + setPrecisionAtK === 0
      ? 0
      : (2 * recallAtK * setPrecisionAtK) / (recallAtK + setPrecisionAtK);

  const firstRelevantIndex = retrievedSkillIds.findIndex((skillId) =>
    gold.has(skillId),
  );
  const idealDcg = idealDiscountedGain(gold.size, k);
  const retrievedCardinality = selected.length;

  return {
    instanceId: record.instance_id,
    k,
    hits,
    recallAtK,
    setPrecisionAtK,
    setF1AtK,
    reciprocalRank: firstRelevantIndex < 0 ? 0 : 1 / (firstRelevantIndex + 1),
    ndcgAtK: discountedGain(selected, gold) / idealDcg,
    exactMatch: hits === gold.size && selected.length === gold.size,
    goldCardinality: gold.size,
    retrievedCardinality,
    cardinalityMatch: retrievedCardinality === gold.size,
    cardinalityAbsoluteError: Math.abs(retrievedCardinality - gold.size),
  };
};

const average = (values: readonly number[]): number =>
  values.reduce((sum, value) => sum + value, 0) / values.length;

const assertUniqueInstances = (
  records: readonly SraRetrievalRecord[],
): void => {
  const instanceIds = records.map((record) => record.instance_id);

  const duplicate = instanceIds.find(
    (instanceId, index) => instanceIds.indexOf(instanceId) !== index,
  );

  if (duplicate) {throw new Error(`duplicate instance_id: ${duplicate}`);}
};

/** Macro-averages retrieval metrics over a non-empty collection of queries. */
export const aggregateSraRetrievalMetrics = (
  records: readonly SraRetrievalRecord[],
  k: number,
): SraAggregateRetrievalMetrics => {
  assertPositiveInteger(k, 'k');

  if (records.length === 0) {
    throw new Error('SRA-Bench metrics require at least one retrieval record.');
  }

  assertUniqueInstances(records);

  const perQuery = records.map((record) => scoreSraRetrieval(record, k));

  return {
    k,
    queryCount: perQuery.length,
    recallAtK: average(perQuery.map((metrics) => metrics.recallAtK)),
    setPrecisionAtK: average(
      perQuery.map((metrics) => metrics.setPrecisionAtK),
    ),
    setF1AtK: average(perQuery.map((metrics) => metrics.setF1AtK)),
    mrr: average(perQuery.map((metrics) => metrics.reciprocalRank)),
    ndcgAtK: average(perQuery.map((metrics) => metrics.ndcgAtK)),
    exactMatchRate: average(
      perQuery.map((metrics) => Number(metrics.exactMatch)),
    ),
    cardinalityAccuracy: average(
      perQuery.map((metrics) => Number(metrics.cardinalityMatch)),
    ),
    meanAbsoluteCardinalityError: average(
      perQuery.map((metrics) => metrics.cardinalityAbsoluteError),
    ),
    perQuery,
  };
};
