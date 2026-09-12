import assert from 'node:assert/strict';
import test from 'node:test';

import {
  aggregateSraRetrievalMetrics,
  scoreSraRetrieval,
  type SraRetrievalRecord,
} from '../src/composition/sra-metrics.js';

const closeTo = (actual: number, expected: number): void => {
  assert.ok(
    Math.abs(actual - expected) < 1e-12,
    `expected ${actual} to be close to ${expected}`,
  );
};

const record = (
  instanceId: string,
  goldSkillIds: readonly string[],
  retrievedSkillIds: readonly string[],
): SraRetrievalRecord => ({
  instance_id: instanceId,
  gold_skill_ids: goldSkillIds,
  retrieved: retrievedSkillIds.map((skillId, index) => ({
    skill_id: skillId,
    score: 1 - index / 10,
  })),
});

test('computes recall, set metrics, MRR, nDCG, and cardinality at K', () => {
  const metrics = scoreSraRetrieval(
    record('query-1', ['a', 'b'], ['a', 'x', 'b']),
    2,
  );

  assert.deepEqual(
    {
      hits: metrics.hits,
      recallAtK: metrics.recallAtK,
      setPrecisionAtK: metrics.setPrecisionAtK,
      setF1AtK: metrics.setF1AtK,
      reciprocalRank: metrics.reciprocalRank,
      exactMatch: metrics.exactMatch,
      goldCardinality: metrics.goldCardinality,
      retrievedCardinality: metrics.retrievedCardinality,
      cardinalityMatch: metrics.cardinalityMatch,
      cardinalityAbsoluteError: metrics.cardinalityAbsoluteError,
    },
    {
      hits: 1,
      recallAtK: 0.5,
      setPrecisionAtK: 0.5,
      setF1AtK: 0.5,
      reciprocalRank: 1,
      exactMatch: false,
      goldCardinality: 2,
      retrievedCardinality: 2,
      cardinalityMatch: true,
      cardinalityAbsoluteError: 0,
    },
  );

  closeTo(metrics.ndcgAtK, 1 / (1 + 1 / Math.log2(3)));
});

test('treats exact match as order-independent set equality', () => {
  const metrics = scoreSraRetrieval(
    record('query-1', ['a', 'b'], ['b', 'a']),
    2,
  );

  assert.deepEqual(
    [
      metrics.recallAtK,
      metrics.setPrecisionAtK,
      metrics.setF1AtK,
      metrics.ndcgAtK,
      metrics.exactMatch,
    ],
    [1, 1, 1, 1, true],
  );
});

test('macro-averages pure per-query retrieval metrics', () => {
  const summary = aggregateSraRetrievalMetrics(
    [
      record('query-1', ['a', 'b'], ['a', 'b']),
      record('query-2', ['c', 'd'], ['x', 'c']),
    ],
    2,
  );

  assert.deepEqual(
    {
      k: summary.k,
      queryCount: summary.queryCount,
      recallAtK: summary.recallAtK,
      setPrecisionAtK: summary.setPrecisionAtK,
      setF1AtK: summary.setF1AtK,
      mrr: summary.mrr,
      exactMatchRate: summary.exactMatchRate,
      cardinalityAccuracy: summary.cardinalityAccuracy,
      meanAbsoluteCardinalityError: summary.meanAbsoluteCardinalityError,
    },
    {
      k: 2,
      queryCount: 2,
      recallAtK: 0.75,
      setPrecisionAtK: 0.75,
      setF1AtK: 0.75,
      mrr: 0.75,
      exactMatchRate: 0.5,
      cardinalityAccuracy: 1,
      meanAbsoluteCardinalityError: 0,
    },
  );

  closeTo(summary.ndcgAtK, (1 + 1 / Math.log2(3) / (1 + 1 / Math.log2(3))) / 2);
});

test('rejects invalid cutoffs, empty batches, and duplicate identifiers', () => {
  assert.throws(
    () => scoreSraRetrieval(record('query-1', ['a'], ['a']), 0),
    /positive integer/,
  );

  assert.throws(
    () => aggregateSraRetrievalMetrics([], 2),
    /at least one retrieval record/,
  );

  assert.throws(
    () => scoreSraRetrieval(record('query-1', ['a', 'a'], ['a']), 2),
    /duplicate gold skill_id: a/,
  );

  assert.throws(
    () => scoreSraRetrieval(record('query-1', ['a'], ['a', 'a']), 2),
    /duplicate retrieved skill_id: a/,
  );
});
