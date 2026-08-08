import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';

import { cli, exists, fail, writeJson } from './study-runtime.mjs';
import { runAndScore } from './study-preparation.mjs';

const schedulePath = (context, family) =>
  join(context.paths.schedules, `${family}.json`);

const transformedId = (context, run, family) =>
  `run.${family}.${context.api.core
    .contentHash({ parent: run.id, family })
    .slice(0, 32)}`;

/** Derives all frozen families without changing their paired seed ledger. */
export const createFamilySchedules = async (context, prepared, manifest) => {
  const primary = prepared.schedule.map((run) =>
    context.api.RunSpecV1.parse({
      ...run,
      freezeHash: manifest.manifestHash,
    }),
  );
  const transform = (family, patch) =>
    primary.map((run) =>
      context.api.RunSpecV1.parse({
        ...run,
        id: transformedId(context, run, family),
        ...patch,
      }),
    );
  const values = {
    primary,
    replication: transform('replication', {
      phase: 'replication',
      model: manifest.replication.candidate,
    }),
    sensitivity: transform('sensitivity', {
      modelCallBudget: manifest.modelCallBudgetP95,
    }),
  };
  for (const [family, schedule] of Object.entries(values)) {
    if (
      context.api.study.seedLedgerHash(schedule) !==
      manifest.artifactHashes.seeds
    ) {
      fail(`${family} schedule changed the frozen seed ledger`);
    }
    await writeJson(context, schedulePath(context, family), schedule);
  }
  return values;
};

export const executeFamilies = async (context) => {
  const freeze = join(context.paths.freeze, 'freeze.json');
  for (const family of ['primary', 'replication', 'sensitivity']) {
    await runAndScore(context, {
      name: family,
      root: context.paths[family],
      schedule: schedulePath(context, family),
      cases: context.config.files.confirmatoryCases,
      freeze,
      family,
    });
  }
};

const copyPowerResult = async (context) => {
  const source = join(context.paths.power, 'power-result.json');
  const destination = join(context.paths.analysisConfig, 'power-result.json');
  const bytes = await readFile(source);
  if (await exists(destination)) {
    if (!bytes.equals(await readFile(destination))) {
      fail('analysis power-result copy differs from frozen bytes');
    }
    return;
  }
  await mkdir(dirname(destination), { recursive: true });
  await writeFile(destination, bytes, { flag: 'wx', mode: 0o600 });
};

const baseAnalysisConfig = (context, manifest, powerResult) => ({
  studyId: context.config.studyId,
  baselineCondition: manifest.selectedBaseline,
  effort: 'medium',
  implementationHash: manifest.artifactHashes.analysis,
  powerResultPath: '/input/config/power-result.json',
  powerParameters: {
    seed: powerResult.seed,
    numericSeed: powerResult.numericSeed,
    baselineProbability: powerResult.baselineProbability,
    randomInterceptSd: powerResult.randomInterceptSd,
    adaptiveClasses: powerResult.adaptiveClasses,
  },
  freeze: {
    manifestHash: manifest.manifestHash,
    nPower: manifest.nPower,
    nFinal: manifest.nFinal,
    repetitions: manifest.repetitions,
    minimumEffect: manifest.minimumEffect,
    modelCallBudgetP95: manifest.modelCallBudgetP95,
    artifactHashes: {
      analysis: manifest.artifactHashes.analysis,
      powerConfig: manifest.artifactHashes.powerConfig,
      powerResult: manifest.artifactHashes.powerResult,
    },
  },
  bootstrapRepetitions: 10_000,
  bootstrapSeed: powerResult.numericSeed,
});

const analysisConfigs = (context, manifest, powerResult) => {
  const base = baseAnalysisConfig(context, manifest, powerResult);
  return {
    primary: {
      ...base,
      family: 'primary',
      provider: manifest.primaryModel.provider,
      model: manifest.primaryModel.model,
    },
    replication: {
      ...base,
      family: 'replication',
      provider: manifest.replication.candidate.provider,
      model: manifest.replication.candidate.model,
    },
    sensitivity: {
      ...base,
      family: 'sensitivity',
      provider: manifest.primaryModel.provider,
      model: manifest.primaryModel.model,
      modelCallBudgetP95: manifest.modelCallBudgetP95,
    },
  };
};

export const analyzeFamilies = async (context, manifest, powerResult) => {
  await copyPowerResult(context);
  for (const [family, config] of Object.entries(
    analysisConfigs(context, manifest, powerResult),
  )) {
    const configPath = join(context.paths.analysisConfig, `${family}.json`);
    const resultPath = join(context.paths.final, `${family}.json`);
    const reportPath = join(context.paths.final, `${family}.md`);
    await writeJson(context, configPath, config);
    await cli({
      label: `${family} R analysis`,
      args: [
        'analyze',
        '--scores',
        join(context.paths[family], 'scores.csv'),
        '--config',
        configPath,
        '--freeze',
        join(context.paths.freeze, 'freeze.json'),
        '--image',
        context.config.analysisImage,
        '--work-dir',
        join(context.paths[family], 'verification'),
        '--result',
        resultPath,
        '--report',
        reportPath,
      ],
      receipt: join(context.paths[family], 'analyze-command.json'),
      outputs: [resultPath, reportPath],
    });
  }
};

export const writeFinalResult = async (context, manifest) => {
  const result = {
    schemaVersion: 1,
    studyId: context.config.studyId,
    freezeHash: manifest.manifestHash,
    selectedBaseline: manifest.selectedBaseline,
    nFinal: manifest.nFinal,
    results: Object.fromEntries(
      ['primary', 'replication', 'sensitivity'].map((family) => [
        family,
        {
          result: join(context.paths.final, `${family}.json`),
          report: join(context.paths.final, `${family}.md`),
          scores: join(context.paths[family], 'scores.csv'),
        },
      ]),
    ),
    nextSteps: [
      'complete the separate blinded 14-day human review protocol',
      'create and audit an explicit publication package plan',
    ],
  };
  await writeJson(context, join(context.paths.final, 'results.json'), result);
  return result;
};
