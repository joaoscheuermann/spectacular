import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  cli,
  materializeInputs,
  parseArgs,
  pathsFor,
} from '../scripts/study-runtime.mjs';
import { createFamilySchedules } from '../scripts/study-confirmatory.mjs';
import { createPrefreezeSchedule } from '../scripts/study-preparation.mjs';

const temporaryDirectory = async (context) => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-study-test-'));
  context.after(() => rm(root, { recursive: true, force: true }));
  return root;
};

test('argument modes are explicit and mutually exclusive', () => {
  assert.deepEqual(parseArgs(['--config', '/study.json', '--yes-paid-study']), {
    config: '/study.json',
    validate: undefined,
    paid: true,
    help: false,
    print: false,
  });
  assert.throws(
    () => parseArgs(['--config', '--yes-paid-study']),
    /--config requires a path/u,
  );
  assert.throws(
    () => parseArgs(['--print-config', '--yes-paid-study']),
    /cannot be combined/u,
  );
  assert.throws(
    () => parseArgs(['--config', '/one.json', '--config', '/two.json']),
    /duplicate argument/u,
  );
});

test('scientific inputs are copied once and source drift is rejected', async (context) => {
  const root = await temporaryDirectory(context);
  const authored = join(root, 'authored');
  const study = join(root, 'study');
  const configPath = join(authored, 'study.json');
  const files = {
    prices: join(authored, 'prices.json'),
    calibrationCases: join(authored, 'calibration-cases.json'),
    confirmatoryCases: join(authored, 'confirmatory-cases.json'),
    powerConfig: join(authored, 'power-config.json'),
  };
  await mkdir(authored);
  await Promise.all([
    writeFile(configPath, '{"schemaVersion":1}\n'),
    ...Object.entries(files).map(([name, path]) =>
      writeFile(path, `${JSON.stringify({ name })}\n`),
    ),
  ]);
  const paths = pathsFor(study);
  const config = { files };
  const pinned = await materializeInputs(config, configPath, paths);

  assert.equal(
    await readFile(pinned.files.prices, 'utf8'),
    `${JSON.stringify({ name: 'prices' })}\n`,
  );
  await writeFile(files.prices, '{"changed":true}\n');
  await assert.rejects(
    materializeInputs(config, configPath, paths),
    /immutable study input differs/u,
  );
});

test('a matching receipt and outputs make a CLI step resumable', async (context) => {
  const root = await temporaryDirectory(context);
  const receipt = join(root, 'score-command.json');
  const output = join(root, 'scores.json');
  await Promise.all([
    writeFile(
      receipt,
      `${JSON.stringify({
        schemaVersion: 1,
        ok: true,
        command: 'score',
        result: {},
      })}\n`,
    ),
    writeFile(output, '[]\n'),
  ]);

  const value = await cli({
    label: 'score',
    args: ['score'],
    receipt,
    outputs: [output],
  });
  assert.equal(value.command, 'score');
});

test('orphan outputs and incomplete receipts are never trusted', async (context) => {
  const root = await temporaryDirectory(context);
  const receipt = join(root, 'command.json');
  const output = join(root, 'result.json');
  await writeFile(output, '{}\n');
  await assert.rejects(
    cli({
      label: 'orphan',
      args: ['not-executed'],
      receipt,
      outputs: [output],
    }),
    /output but no immutable receipt/u,
  );

  await rm(output);
  await writeFile(
    receipt,
    `${JSON.stringify({
      schemaVersion: 1,
      ok: true,
      command: 'not-executed',
      result: {},
    })}\n`,
  );
  await assert.rejects(
    cli({
      label: 'incomplete',
      args: ['not-executed'],
      receipt,
      outputs: [output],
    }),
    /receipt but lacks an expected output/u,
  );
});

test('the confirmatory schedule contains one baseline and one M1 condition', () => {
  let received;
  const baseline = { id: 'B2' };
  const m1 = { id: 'M1' };
  const context = {
    config: {
      studyId: 'study-test',
      seeds: { confirmatorySchedule: 'confirmatory-seed' },
    },
    api: {
      conditions: {
        M1: m1,
        conditionById: (id) => {
          assert.equal(id, 'B2');
          return baseline;
        },
      },
      study: {
        createSchedule: (input) => {
          received = input;
          return [];
        },
      },
    },
  };

  createPrefreezeSchedule(context, [{ id: 'case-001' }], 'B2');
  assert.deepEqual(received.conditions, [baseline, m1]);
  assert.equal(new Set(received.conditions.map(({ id }) => id)).size, 2);
  assert.equal(received.repetitions, 5);
  assert.equal(received.capture, 'structure');
  assert.equal(received.freezeHash, null);
});

test('derived families preserve seeds while changing only family fields', async (context) => {
  const root = await temporaryDirectory(context);
  const ledger = (schedule) =>
    JSON.stringify(
      schedule.map(
        ({
          studyId,
          caseId,
          conditionId,
          repetition,
          seed,
          pairedBlock,
          order,
        }) => ({
          studyId,
          caseId,
          conditionId,
          repetition,
          seed,
          pairedBlock,
          order,
        }),
      ),
    );
  const schedule = ['B2', 'M1'].map((conditionId, order) => ({
    schemaVersion: 1,
    id: `run-${conditionId}`,
    studyId: 'study-test',
    phase: 'confirmatory',
    caseId: 'case-001',
    conditionId,
    repetition: 1,
    seed: 42,
    pairedBlock: 'case-001.rep.1',
    order,
    model: { provider: 'openai', model: 'luna', effort: 'medium' },
    capture: 'structure',
    freezeHash: null,
  }));
  const manifest = {
    manifestHash: `sha256:${'a'.repeat(64)}`,
    modelCallBudgetP95: 7,
    artifactHashes: { seeds: ledger(schedule) },
    replication: {
      candidate: {
        provider: 'openrouter',
        model: 'qwen',
        effort: 'medium',
      },
    },
  };
  const contextValue = {
    paths: { schedules: join(root, 'schedules') },
    api: {
      RunSpecV1: { parse: (value) => value },
      core: {
        canonicalJson: JSON.stringify,
        contentHash: (value) =>
          createHash('sha256').update(JSON.stringify(value)).digest('hex'),
      },
      study: { seedLedgerHash: ledger },
    },
  };

  const families = await createFamilySchedules(
    contextValue,
    { schedule },
    manifest,
  );
  assert.deepEqual(
    families.primary.map(({ freezeHash }) => freezeHash),
    [manifest.manifestHash, manifest.manifestHash],
  );
  assert.ok(
    families.replication.every(
      ({ phase, model }) =>
        phase === 'replication' && model === manifest.replication.candidate,
    ),
  );
  assert.ok(
    families.sensitivity.every(
      ({ phase, modelCallBudget }) =>
        phase === 'confirmatory' && modelCallBudget === 7,
    ),
  );
  assert.equal(new Set(families.replication.map(({ id }) => id)).size, 2);
  assert.equal(new Set(families.sensitivity.map(({ id }) => id)).size, 2);
});
