import assert from 'node:assert/strict';
import test from 'node:test';

import {
  parseSraCorpus,
  parseSraCorpusJson,
  parseSraInstances,
  parseSraInstancesJson,
} from '../src/composition/sra-fixtures.js';

const corpusFixture = [
  {
    skill_id: 'champ_001',
    name: 'Counting paths',
    description: 'Counts constrained paths.',
    content: 'Apply the recurrence.',
    tools: null,
  },
  {
    skill_id: 'bigcodebench_001',
    name: 'Python iteration',
    description: 'Iterates over Python collections.',
    content: 'Use a comprehension when appropriate.',
  },
] as const;

const instanceFixture = [
  {
    instance_id: 'champ_00001',
    dataset: 'champ',
    question: 'How many paths satisfy the constraint?',
    skill_annotations: ['champ_001', 'champ_002'],
    eval_data: { answer: '42' },
  },
] as const;

test('parses downloaded SRA-Bench JSON without stripping extra fields', () => {
  const corpus = parseSraCorpusJson(JSON.stringify(corpusFixture));
  const instances = parseSraInstancesJson(JSON.stringify(instanceFixture));

  assert.equal(corpus[0]?.tools, null);

  assert.deepEqual(instances[0]?.eval_data, { answer: '42' });

  assert.deepEqual(parseSraCorpus(corpusFixture), corpus);

  assert.deepEqual(parseSraInstances(instanceFixture), instances);
});

test('rejects malformed JSON and missing required fixture fields', () => {
  assert.throws(() => parseSraCorpusJson('{'), /valid JSON/);

  assert.throws(
    () =>
      parseSraInstances([
        {
          instance_id: 'champ_00001',
          dataset: 'champ',
          skill_annotations: ['champ_001'],
        },
      ]),
    /question/,
  );
});

test('rejects duplicate corpus, instance, and annotation identifiers', () => {
  assert.throws(
    () => parseSraCorpus([corpusFixture[0], corpusFixture[0]]),
    /duplicate skill_id: champ_001/,
  );

  assert.throws(
    () => parseSraInstances([instanceFixture[0], instanceFixture[0]]),
    /duplicate instance_id: champ_00001/,
  );

  assert.throws(
    () =>
      parseSraInstances([
        {
          ...instanceFixture[0],
          skill_annotations: ['champ_001', 'champ_001'],
        },
      ]),
    /duplicate skill annotation: champ_001/,
  );
});
