import assert from 'node:assert/strict';
import test from 'node:test';

import { parseSraRetrievalResults } from '../src/composition/sra-artifacts.js';

test('parses the official retrieval interchange format', () => {
  const records = parseSraRetrievalResults(
    JSON.stringify({
      metadata: { dataset: 'champ', retriever: 'bm25' },
      results: [
        {
          instance_id: 'champ_1',
          gold_skill_ids: ['a', 'b'],
          retrieved: [
            { skill_id: 'a', score: 2 },
            { skill_id: 'noise', score: 1 },
          ],
        },
      ],
    }),
  );

  assert.deepEqual(records, [
    {
      instance_id: 'champ_1',
      gold_skill_ids: ['a', 'b'],
      retrieved: [
        { skill_id: 'a', score: 2 },
        { skill_id: 'noise', score: 1 },
      ],
    },
  ]);
});

test('rejects duplicate instances and malformed rankings', () => {
  const record = {
    instance_id: 'champ_1',
    gold_skill_ids: ['a'],
    retrieved: [{ skill_id: 'a', score: 1 }],
  };
  assert.throws(
    () =>
      parseSraRetrievalResults(JSON.stringify({ results: [record, record] })),
    /duplicate instances/,
  );
  assert.throws(
    () =>
      parseSraRetrievalResults(
        JSON.stringify({ results: [{ ...record, retrieved: [{}] }] }),
      ),
    /skill_id/,
  );
});
