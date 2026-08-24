import assert from 'node:assert/strict';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import type { LlmProvider } from 'llms';

import type { CompositionRunInput } from '../src/composition/runner.js';
import { runSraDataset } from '../src/composition/sra-run.js';
import {
  parseSraCorpus,
  parseSraInstances,
} from '../src/composition/sra-fixtures.js';

const profile = {
  provider: {} as LlmProvider,
  model: 'test-model',
};

const instances = parseSraInstances([
  {
    instance_id: 'champ_00001',
    dataset: 'champ',
    question: 'First question?',
    skill_annotations: ['champ_a', 'champ_b'],
  },
  {
    instance_id: 'champ_00002',
    dataset: 'champ',
    question: 'Second question?',
    skill_annotations: ['champ_b', 'champ_c'],
  },
]);

const corpus = parseSraCorpus(
  ['champ_a', 'champ_b', 'champ_c', 'noise'].map((skillId) => ({
    skill_id: skillId,
    name: `Name ${skillId}`,
    description: `Description ${skillId}`,
    content: `Instructions ${skillId}`,
  })),
);

const retrieval = instances.map((instance) => ({
  instance_id: instance.instance_id,
  gold_skill_ids: instance.skill_annotations,
  retrieved: [
    { skill_id: 'noise', score: 4 },
    { skill_id: instance.skill_annotations[0]!, score: 3 },
    { skill_id: instance.skill_annotations[1]!, score: 2 },
  ],
}));

test('writes official inference JSONL while preserving the frozen ranking', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-sra-run-'));
  const outputPath = join(root, 'fixed.jsonl');
  const inputs: CompositionRunInput[] = [];

  try {
    const result = await runSraDataset(
      {
        arm: 'fixed-top-k',
        instances,
        corpus,
        retrieval,
        profile,
        outputPath,
        topK: 2,
      },
      {
        runCase: async (input) => {
          inputs.push(input);
          return {
            id: input.benchmarkCase.id,
            dataset: input.benchmarkCase.dataset,
            arm: input.arm,
            status: 'completed',
            rawOutput: 'ANSWER: 42',
            skillIdsUsed: ['noise', 'champ_a'],
            candidateSkillIds: [],
          };
        },
      },
    );
    const records = (await readFile(outputPath, 'utf8'))
      .trim()
      .split('\n')
      .map((line) => JSON.parse(line) as Record<string, unknown>);

    assert.deepEqual(result, {
      arm: 'fixed-top-k',
      dataset: 'champ',
      outputPath,
      completed: 2,
      skipped: 0,
      failed: 0,
      runSha256: result.runSha256,
      manifestPath: `${outputPath}.run.json`,
    });
    assert.match(result.runSha256, /^[a-f0-9]{64}$/u);
    assert.deepEqual(
      inputs[0]?.ranking.map(({ skillId }) => skillId),
      ['noise', 'champ_a', 'champ_b'],
    );
    assert.deepEqual(records[0], {
      instance_id: 'champ_00001',
      dataset: 'champ',
      method: 'mosaic_fixed_top_k',
      model: 'test-model',
      raw_output: 'ANSWER: 42',
      skill_ids_used: ['noise', 'champ_a'],
      meta: { workflow_status: 'completed', candidate_skill_ids: [] },
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('resumes complete records without invoking the model again', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-sra-resume-'));
  const outputPath = join(root, 'oracle.jsonl');
  let calls = 0;

  try {
    const options = {
      arm: 'oracle' as const,
      instances: instances.slice(0, 1),
      corpus,
      retrieval: [],
      profile,
      outputPath,
    };
    const dependencies = {
      runCase: async (input: CompositionRunInput) => {
        calls += 1;
        return {
          id: input.benchmarkCase.id,
          dataset: input.benchmarkCase.dataset,
          arm: input.arm,
          status: 'completed' as const,
          rawOutput: 'ANSWER: 1',
          skillIdsUsed: [...input.benchmarkCase.goldSkillIds],
          candidateSkillIds: [],
        };
      },
    };

    await runSraDataset(options, dependencies);
    const resumed = await runSraDataset(options, dependencies);

    assert.equal(calls, 1);
    assert.equal(resumed.completed, 0);
    assert.equal(resumed.skipped, 1);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('rejects mixed datasets and rankings whose gold set changed', async () => {
  await assert.rejects(
    () =>
      runSraDataset({
        arm: 'no-skills',
        instances: [instances[0]!, { ...instances[1]!, dataset: 'other' }],
        corpus,
        retrieval: [],
        profile,
        outputPath: 'unused.jsonl',
      }),
    /one dataset/,
  );
  await assert.rejects(
    () =>
      runSraDataset({
        arm: 'mosaic',
        instances: instances.slice(0, 1),
        corpus,
        retrieval: [{ ...retrieval[0]!, gold_skill_ids: ['champ_a'] }],
        profile,
        outputPath: 'unused.jsonl',
      }),
    /gold skills differ/,
  );
});

test('refuses to resume output under a different closed run identity', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-sra-identity-'));
  const outputPath = join(root, 'mosaic.jsonl');
  try {
    const base = {
      arm: 'oracle' as const,
      instances: instances.slice(0, 1),
      corpus,
      retrieval: [],
      profile,
      outputPath,
    };
    await runSraDataset(base, {
      runCase: async (input) => ({
        id: input.benchmarkCase.id,
        dataset: input.benchmarkCase.dataset,
        arm: input.arm,
        status: 'completed',
        rawOutput: 'ANSWER: 1',
        skillIdsUsed: [...input.benchmarkCase.goldSkillIds],
        candidateSkillIds: [],
      }),
    });

    await assert.rejects(
      () =>
        runSraDataset(
          { ...base, maxTurns: 17 },
          {
            runCase: async () => {
              throw new Error('must not run');
            },
          },
        ),
      /different run identity/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('refuses a mixed or mutated inference record during resume', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-sra-record-'));
  const outputPath = join(root, 'oracle.jsonl');
  const options = {
    arm: 'oracle' as const,
    instances: instances.slice(0, 1),
    corpus,
    retrieval: [],
    profile,
    outputPath,
  };
  try {
    await runSraDataset(options, {
      runCase: async (input) => ({
        id: input.benchmarkCase.id,
        dataset: input.benchmarkCase.dataset,
        arm: input.arm,
        status: 'completed',
        rawOutput: 'ANSWER: 1',
        skillIdsUsed: [...input.benchmarkCase.goldSkillIds],
        candidateSkillIds: [],
      }),
    });
    const record = JSON.parse(await readFile(outputPath, 'utf8')) as Record<
      string,
      unknown
    >;
    await writeFile(
      outputPath,
      `${JSON.stringify({ ...record, model: 'different-model' })}\n`,
      'utf8',
    );

    await assert.rejects(
      runSraDataset(options, {
        runCase: async () => {
          throw new Error('must not run');
        },
      }),
      /record has a different identity/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
