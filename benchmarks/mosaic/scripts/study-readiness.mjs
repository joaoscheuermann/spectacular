import { statfs } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import pino from 'pino';
import { z } from 'zod';

import {
  benchmarkRoot,
  cleanCommit,
  execute,
  exists,
  loadApi,
  readJson,
  validateConfig,
} from './study-runtime.mjs';
import { pathsFor } from './study-inputs.mjs';
import {
  validateHumanApproval,
  validatePowerApproval,
} from './study-approvals.mjs';

const fixed = {
  git: 'repository commit or worktree is not ready',
  image: 'immutable analysis image or its offline R test is unavailable',
  instrument: 'instrument validation or conformance failed',
  calibration: 'calibration corpus or its human audit is not ready',
  confirmatory: 'confirmatory corpus or its human audit is not ready',
  prices: 'price snapshot or cost approval is not ready',
  power: 'power config, approval, or preparation result is not ready',
  index: 'paid setup index is absent or does not match its immutable artifact',
  capabilities: 'free model metadata does not verify every required capability',
  credential: 'OPENROUTER_API_KEY is unavailable in the runtime environment',
  disk: 'artifact root does not have the approved free disk capacity',
  schemas: 'checked-in JSON Schemas do not match the current schema inventory',
};

const issue = (code, kind = 'implementation') => ({
  priority: 'P0',
  kind,
  code,
  message: fixed[code],
});

const existingParent = async (path) => {
  let current = resolve(path);
  while (!(await exists(current))) {
    const parent = dirname(current);
    if (parent === current) return current;
    current = parent;
  }
  return current;
};

const privateModule = (relative) =>
  import(pathToFileURL(join(benchmarkRoot, 'dist', 'cli', relative)).href);

/** Runs every no-cost readiness gate and reports all independent failures. */
export const checkReadiness = async (configPath, stage) => {
  let config;
  try {
    config = await validateConfig(await readJson(configPath), stage);
  } catch {
    return {
      schemaVersion: 1,
      stage,
      envelopeValid: false,
      stageReady: false,
      studyReady: false,
      issues: [
        {
          priority: 'P1',
          kind: 'input',
          code: 'envelope',
          message: 'study config envelope or referenced input is invalid',
        },
      ],
    };
  }

  const issues = [];
  const check = async (code, operation, kind) => {
    try {
      await operation();
    } catch {
      issues.push(issue(code, kind));
    }
  };
  const paths = pathsFor(resolve(config.root));
  let api;
  await check('instrument', async () => {
    api = await loadApi();
    const validation = api.core.HARNESS_COMMANDS.validate();
    const conformance = await api.core.HARNESS_COMMANDS.conformance();
    if (!validation.valid || conformance.length > 0) throw new Error('invalid');
  });

  await check('git', async () => void (await cleanCommit()), 'operational');
  await check(
    'image',
    async () => {
      await execute('docker', ['image', 'inspect', config.analysisImage]);
      await execute('docker', [
        'run',
        '--rm',
        '--pull',
        'never',
        '--network',
        'none',
        '--read-only',
        '--cap-drop',
        'ALL',
        '--security-opt',
        'no-new-privileges',
        '--tmpfs',
        '/tmp:rw,noexec,nosuid,size=256m',
        config.analysisImage,
        '/benchmark/analysis/test-analysis.R',
      ]);
    },
    'operational',
  );

  let prices;
  let powerApproval;
  if (api !== undefined) {
    await check(
      'calibration',
      async () => {
        const values = (await readJson(config.files.calibrationCases)).map(
          (value) => api.CaseV1.parse(value),
        );
        const found = api.study.validateCalibrationCases(
          values,
          api.study.PILOT_CASES,
        );
        if (found.length > 0) throw new Error('invalid');
        validateHumanApproval(
          { api },
          await readJson(config.files.calibrationAudit),
          {
            subject: 'calibration-corpus',
            artifact: values,
            checks: [
              'english',
              'neutrality',
              'difficulty',
              'no-answer-marker',
              'procedural-equivalence',
              'overlap',
              'distractors',
              'conflicts',
              'phase-isolation',
            ],
            maximumCost: false,
          },
        );
      },
      'human',
    );

    await check(
      'prices',
      async () => {
        const pricing = await privateModule('pricing.js');
        prices = pricing.parsePrices(await readJson(config.files.prices));
        pricing.validatePriceCoverage(prices, [
          { model: api.config.PRIMARY_MODEL.model, kind: 'completion' },
          { model: config.candidate.model, kind: 'completion' },
          { model: api.config.EMBEDDING_MODEL.model, kind: 'embedding' },
          { model: api.config.RERANKER_MODEL, kind: 'rerank' },
        ]);
        validateHumanApproval(
          { api },
          await readJson(config.files.costApproval),
          {
            subject: 'study-cost',
            artifact: await readJson(config.files.prices),
            checks: [
              'setup-cost-included',
              'run-cost-estimated',
              'credit-confirmed',
              'rate-limits-confirmed',
              'maximum-cost-approved',
            ],
            maximumCost: true,
          },
        );
      },
      'human',
    );

    await check(
      'power',
      async () => {
        const powerConfig = await readJson(config.files.powerConfig);
        powerApproval = await readJson(config.files.powerApproval);
        validatePowerApproval({ api }, powerConfig, powerApproval);
        if (stage === 'continue') {
          const preparation = await readJson(
            join(paths.power, 'preparation.json'),
          );
          const result = api.PowerResultV1.parse(
            await readJson(join(paths.power, 'power-result.json')),
          );
          if (
            preparation.studyId !== config.studyId ||
            preparation.nFinal !== result.nFinal ||
            preparation.powerResultHash !== api.core.artifactHash(result)
          ) {
            throw new Error('invalid');
          }
        }
      },
      'human',
    );

    if (stage === 'continue') {
      await check(
        'confirmatory',
        async () => {
          const preparation = await readJson(
            join(paths.power, 'preparation.json'),
          );
          const calibration = (
            await readJson(config.files.calibrationCases)
          ).map((value) => api.CaseV1.parse(value));
          const values = (await readJson(config.files.confirmatoryCases)).map(
            (value) => api.CaseV1.parse(value),
          );
          const found = [
            ...api.study.validateConfirmatoryCases(
              values,
              api.study.PILOT_CASES,
              preparation.nFinal,
            ),
            ...api.study.validateFamilyIsolation(calibration, values),
          ];
          if (found.length > 0) throw new Error('invalid');
          validateHumanApproval(
            { api },
            await readJson(config.files.confirmatoryAudit),
            {
              subject: 'confirmatory-corpus',
              artifact: values,
              checks: [
                'english',
                'neutrality',
                'independence',
                'no-answer-marker',
                'balance',
                'phase-isolation',
              ],
              maximumCost: false,
            },
          );
        },
        'human',
      );
    }
  }

  await check('schemas', async () => {
    if (api === undefined) throw new Error('api unavailable');
    for (const [name, schema] of Object.entries(api.schemasV1)) {
      const path = join(benchmarkRoot, 'schemas', 'v1', `${name}.schema.json`);
      if (!(await exists(path))) {
        throw new Error('missing');
      }
      const generated = z.toJSONSchema(schema, {
        io: 'input',
        unrepresentable: 'any',
      });
      if (
        api.core.artifactHash(await readJson(path)) !==
        api.core.artifactHash(generated)
      ) {
        throw new Error('stale');
      }
    }
  });

  if (stage === 'continue') {
    await check('index', async () => {
      const receipt = await readJson(join(paths.preflight, 'index.json'));
      const result = receipt.result;
      if (
        receipt.ok !== true ||
        receipt.command !== 'index' ||
        !(await exists(result.path))
      ) {
        throw new Error('missing');
      }
      const lifecycle = await privateModule('index-lifecycle.js');
      const artifact = await lifecycle.loadProductionIndex(result.path);
      if (artifact.indexHash !== result.indexHash) {
        throw new Error('mismatch');
      }
    });
  }

  const apiKey = process.env.OPENROUTER_API_KEY;
  if (!apiKey?.trim()) {
    issues.push(issue('credential', 'operational'));
  } else if (prices !== undefined) {
    await check('capabilities', async () => {
      const [{ createStudyProvider }, { validateFreeCapabilities }] =
        await Promise.all([
          privateModule('provider-factory.js'),
          privateModule('preflight.js'),
        ]);
      const provider = createStudyProvider(
        apiKey,
        prices,
        pino({ level: 'silent' }),
      );
      await validateFreeCapabilities(provider.provider, config.candidate.model);
    });
  }

  await check(
    'disk',
    async () => {
      if (powerApproval === undefined) throw new Error('approval unavailable');
      const parent = await existingParent(config.root);
      const stats = await statfs(parent);
      const free = Number(stats.bavail) * Number(stats.bsize);
      const required = powerApproval.resourceEstimate.diskGiB * 1024 ** 3;
      if (!Number.isFinite(free) || free < required)
        throw new Error('insufficient');
    },
    'operational',
  );

  return {
    schemaVersion: 1,
    stage,
    envelopeValid: true,
    stageReady: issues.length === 0,
    studyReady: stage === 'continue' && issues.length === 0,
    issues,
  };
};
