import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

import { parseInvocation } from '../src/cli/args.js';
import { executeCommand } from '../src/cli/commands.js';
import { parsePrices, pricesHash, tokenCost } from '../src/cli/pricing.js';
import { B0 } from '../src/conditions/index.js';
import { TOOL_NAMES } from '../src/config/index.js';
import { artifactHash } from '../src/core/index.js';
import {
  createEventStore,
  createRecordStore,
  executeRun,
} from '../src/runtime/index.js';
import { createSchedule, PILOT_CASES } from '../src/study/index.js';

const executeFile = promisify(execFile);
const hash = `sha256:${'a'.repeat(64)}`;

test('CLI parser keeps flags explicit and rejects duplicates', () => {
  assert.deepEqual(
    parseInvocation(['review', 'status', '--input', 'review.json']),
    {
      command: 'review',
      subcommand: 'status',
      flags: { input: 'review.json' },
    },
  );
  assert.throws(
    () => parseInvocation(['pilot', '--seed', 'one', '--seed', 'two']),
    /duplicate option/u,
  );
});

test('validate command returns all frozen instrument counts', async () => {
  const output = await executeCommand(parseInvocation(['validate']));
  const result = output.result as {
    readonly valid: boolean;
    readonly issues: readonly unknown[];
    readonly counts: Readonly<Record<string, number>>;
    readonly artifactHashes: Readonly<Record<string, string>>;
  };
  assert.equal(result.valid, true);
  assert.deepEqual(result.issues, []);
  assert.deepEqual(result.counts, {
    skills: 60,
    tools: 24,
    pilotCases: 60,
    conditions: 17,
    smokes: 6,
  });
  assert.deepEqual(Object.keys(result.artifactHashes).sort(), [
    'analysis',
    'catalog',
    'conditions',
    'pilotCases',
    'prompts',
    'protocol',
    'renvLock',
    'schemas',
    'tools',
  ]);
  for (const hash of Object.values(result.artifactHashes)) {
    assert.match(hash, /^sha256:[a-f0-9]{64}$/u);
  }
});

test('built CLI writes one JSON document to stdout and progress to stderr', async () => {
  const result = await executeFile(process.execPath, [
    'benchmarks/mosaic/dist/src/cli.js',
    'validate',
  ]);
  const lines = result.stdout.trim().split('\n');
  assert.equal(lines.length, 1);
  const output = JSON.parse(lines[0] as string) as {
    readonly ok: boolean;
    readonly result: { readonly valid: boolean };
  };
  assert.equal(output.ok, true);
  assert.equal(output.result.valid, true);
  assert.match(result.stderr, /validate started/u);
  assert.doesNotMatch(result.stderr, /OPENROUTER_API_KEY/u);
});

test('frozen pricing is hashed and costs cached input separately', () => {
  const prices = parsePrices({
    schemaVersion: 1,
    currency: 'USD',
    capturedAt: '2026-08-08T00:00:00.000Z',
    source: 'https://example.test/prices',
    models: {
      model: {
        inputPerMillion: 2,
        cachedInputPerMillion: 1,
        outputPerMillion: 4,
        perRequest: 0,
      },
    },
  });
  assert.match(pricesHash(prices), /^sha256:[a-f0-9]{64}$/u);
  assert.equal(
    tokenCost(prices, 'model', {
      inputTokens: 1_000_000,
      cachedInputTokens: 500_000,
      outputTokens: 500_000,
    }),
    3.5,
  );
});

test('calibrate-models materializes a self-hashed reproducible artifact', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'mosaic-calibration-'));
  try {
    const inputPath = join(directory, 'input.json');
    const cases = PILOT_CASES.map((source, index) => {
      const ordinal = String(index + 1).padStart(2, '0');
      const { contentHash: _contentHash, ...sourceBody } = source;
      void _contentHash;
      const body = {
        ...sourceBody,
        id: `calibration.case.${ordinal}`,
        familyId: `calibration.family.${ordinal}`,
        phase: 'calibration' as const,
        title: `Neutral calibration ${ordinal}`,
        request: `${source.request}\nIndependent neutral calibration ${ordinal}.`,
        tags: ['calibration', `domain.${source.domain}`],
      };
      return { ...body, contentHash: artifactHash(body) };
    });
    const candidate = {
      provider: 'openrouter',
      model: 'qwen/qwen3.7-flash',
      effort: 'medium' as const,
    };
    const rows = (
      label: string,
      model: {
        readonly provider: string;
        readonly model: string;
        readonly effort: 'medium';
      },
    ) =>
      cases.flatMap((benchmarkCase, caseIndex) =>
        [1, 2, 3].map((repetition) => ({
          schemaVersion: 1,
          runId: `run.calibration.${label}.${caseIndex + 1}.${repetition}`,
          attempt: 1,
          studyId: 'calibration-study',
          phase: 'calibration',
          caseId: benchmarkCase.id,
          familyId: benchmarkCase.familyId,
          conditionId: 'M1',
          repetition,
          pairedBlock: `${benchmarkCase.id}.rep.${repetition}`,
          provider: model.provider,
          model: model.model,
          effort: model.effort,
          freezeHash: null,
          traceRootHash: hash,
          traceDerivedHash: hash,
          evidenceHash: hash,
          worldHash: benchmarkCase.gold.expectedState.worldHash,
          modelCallBudget: null,
          domain: benchmarkCase.domain,
          compositionClass: benchmarkCase.compositionClass,
          adaptive: benchmarkCase.adaptive,
          success: 1,
          primaryEligible: false,
          infrastructure: false,
          failureCode: null,
          retrieval: null,
          bundleExact: true,
          menuExact: true,
          observationExact: true,
          inputTokens: 1,
          outputTokens: 1,
          modelCalls: 1,
          toolCalls: benchmarkCase.gold.requiredTools.length,
          costUsd: 0.01,
          durationMs: 1,
        })),
      );
    await writeFile(
      inputPath,
      JSON.stringify({
        studyId: 'calibration-study',
        candidate,
        seed: 'calibration-seed',
        cases,
        lunaRows: rows('luna', {
          provider: 'openai',
          model: 'openai/gpt-5.6-luna',
          effort: 'medium',
        }),
        candidateRows: rows('candidate', candidate),
      }),
      'utf8',
    );
    const command = await executeCommand(
      parseInvocation(['calibrate-models', '--input', inputPath]),
    );
    const artifact = command.result as Readonly<Record<string, unknown>>;
    const declared = artifact['artifactHash'];
    const { artifactHash: _artifactHash, ...body } = artifact;
    assert.equal(declared, artifactHash(body));
    assert.equal(
      (artifact['result'] as Readonly<Record<string, unknown>>)['approved'],
      true,
    );
    assert.equal((artifact['observations'] as readonly unknown[]).length, 180);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('score derives assertions only from the terminal record and verified trace', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'mosaic-cli-score-'));
  try {
    const benchmarkCase = PILOT_CASES[0]!;
    const run = createSchedule({
      studyId: 'cli-score',
      cases: [benchmarkCase],
      conditions: [B0],
      seed: 'cli-score',
      repetitions: 1,
    })[0]!;
    const events = createEventStore(directory);
    const records = createRecordStore(directory);
    await executeRun(run, benchmarkCase, B0, {
      events,
      records,
      execute: async (context) => {
        await context.emit({
          type: 'baseline.menu',
          skillNames: [],
          toolNames: TOOL_NAMES,
        });
        return {
          status: 'succeeded',
          outcome: {
            goals: [
              {
                output:
                  benchmarkCase.gold.expectedDelivery?.contains.join(' ') ?? '',
              },
            ],
          },
          usage: { inputTokens: 2, outputTokens: 1, costUsd: 0.01 },
        };
      },
    });
    const schedulePath = join(directory, 'schedule.json');
    const casesPath = join(directory, 'cases.json');
    await writeFile(schedulePath, JSON.stringify([run]), 'utf8');
    await writeFile(casesPath, JSON.stringify([benchmarkCase]), 'utf8');

    const command = await executeCommand(
      parseInvocation([
        'score',
        '--schedule',
        schedulePath,
        '--artifacts',
        directory,
        '--cases',
        casesPath,
        '--family',
        'exploratory',
      ]),
    );
    const rows = command.result as readonly {
      readonly success: number;
      readonly pairedBlock: string;
      readonly evidenceHash: string;
    }[];
    assert.equal(rows.length, 1);
    assert.equal(rows[0]?.success, 1);
    assert.equal(rows[0]?.pairedBlock, run.pairedBlock);
    assert.match(rows[0]?.evidenceHash ?? '', /^sha256:[a-f0-9]{64}$/u);

    await assert.rejects(
      executeCommand(
        parseInvocation(['score', '--input', join(directory, 'claims.json')]),
      ),
      /unknown option/u,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
