import { readFile } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';

import { z } from 'zod';

import { artifactHash, sha256 } from '../core/index.js';
import { immutableExists } from '../runtime/index.js';
import {
  AnalysisResultV1,
  PowerResultV1,
  type FreezeManifest,
} from '../schemas/index.js';
import type { CliInvocation } from './args.js';
import { optionalFlag, rejectUnknownFlags, requiredFlag } from './args.js';
import {
  progress,
  readJson,
  writeJsonExclusive,
  writeTextExclusive,
} from './io.js';
import { readAnalysisResult, runContainerVerifier } from './r.js';
import { loadFreezeManifest } from './run.js';
import { fileHash, imageDigest } from './shared.js';

export const power = async (invocation: CliInvocation): Promise<unknown> => {
  rejectUnknownFlags(invocation, ['config', 'image', 'work-dir', 'result']);
  const destination = requiredFlag(invocation, 'result');
  if (await immutableExists(destination)) {
    throw new Error('power result already exists');
  }
  const configPath = requiredFlag(invocation, 'config');
  const configHash = `sha256:${sha256(await readFile(configPath))}`;
  progress('running frozen 10,000-simulation power analysis twice');
  const verificationDirectory = await runContainerVerifier('verify-power.sh', [
    requiredFlag(invocation, 'image'),
    configPath,
    requiredFlag(invocation, 'work-dir'),
  ]);
  const value = PowerResultV1.parse(
    await readAnalysisResult(join(verificationDirectory, 'power-1.json')),
  );
  if (value.configHash !== configHash) {
    throw new Error('power result does not bind the supplied configuration');
  }
  const result = await writeJsonExclusive(destination, value);
  const checksums = await readFile(
    join(verificationDirectory, 'SHA256SUMS'),
    'utf8',
  );
  return {
    result,
    powerConfigHash: configHash,
    powerResultHash: await fileHash(destination),
    verificationDirectory,
    checksumsHash: artifactHash(checksums),
  };
};

const compositionClass = z.enum(['A', 'B', 'C', 'D', 'E', 'F']);

const powerParametersV1 = z
  .object({
    seed: z.string().trim().min(1),
    numericSeed: z.number().int().safe(),
    baselineProbability: z.number().finite().gt(0).lt(1),
    randomInterceptSd: z.number().finite().nonnegative(),
    adaptiveClasses: z.array(compositionClass).min(1),
  })
  .strict()
  .superRefine((value, context) => {
    if (value.seed !== `L'Ecuyer-CMRG:${value.numericSeed}`) {
      context.addIssue({
        code: 'custom',
        message: 'power seed must identify numericSeed',
        path: ['seed'],
      });
    }
    if (new Set(value.adaptiveClasses).size !== value.adaptiveClasses.length) {
      context.addIssue({
        code: 'custom',
        message: 'adaptiveClasses must be unique',
        path: ['adaptiveClasses'],
      });
    }
  });

const secondaryContrastV1 = z
  .object({
    id: z.string().trim().min(1),
    condition: z.literal('M1'),
    reference: z.enum(['B0', 'B1', 'B2', 'B3']),
    classes: z.array(compositionClass).min(1).optional(),
    adaptive: z.boolean().optional(),
  })
  .strict();

const configV1 = z
  .object({
    studyId: z.string().trim().min(1),
    family: z.enum(['primary', 'secondary', 'replication', 'sensitivity']),
    baselineCondition: z.enum(['B0', 'B1', 'B2', 'B3']),
    provider: z.string().trim().min(1),
    model: z.string().trim().min(1),
    effort: z.literal('medium'),
    implementationHash: z.string().trim().min(1),
    powerResultPath: z.string().trim().min(1),
    powerParameters: powerParametersV1,
    freeze: z
      .object({
        manifestHash: z.string().trim().min(1),
        nPower: z.number().int().positive().safe(),
        nFinal: z.number().int().positive().safe(),
        repetitions: z.literal(5),
        minimumEffect: z.literal(0.1),
        modelCallBudgetP95: z.number().int().positive().safe(),
        artifactHashes: z
          .object({
            analysis: z.string().trim().min(1),
            powerConfig: z.string().trim().min(1),
            powerResult: z.string().trim().min(1),
          })
          .strict(),
      })
      .strict(),
    modelCallBudgetP95: z.number().int().positive().safe().optional(),
    bootstrapRepetitions: z.literal(10_000),
    bootstrapSeed: z.number().int().safe(),
    contrasts: z.array(secondaryContrastV1).min(1).optional(),
  })
  .strict();

const validateConfig = (
  value: unknown,
  freeze: FreezeManifest,
): z.infer<typeof configV1> => {
  const config = configV1.parse(value);
  if (
    config.studyId !== freeze.studyId ||
    config.baselineCondition !== freeze.selectedBaseline ||
    config.freeze.manifestHash !== freeze.manifestHash ||
    config.freeze.artifactHashes.analysis !== freeze.artifactHashes.analysis ||
    config.freeze.artifactHashes.powerConfig !==
      freeze.artifactHashes.powerConfig ||
    config.freeze.artifactHashes.powerResult !==
      freeze.artifactHashes.powerResult ||
    config.freeze.nPower !== freeze.nPower ||
    config.freeze.nFinal !== freeze.nFinal ||
    config.freeze.repetitions !== freeze.repetitions ||
    config.freeze.minimumEffect !== freeze.minimumEffect ||
    config.freeze.modelCallBudgetP95 !== freeze.modelCallBudgetP95 ||
    config.implementationHash !== freeze.artifactHashes.analysis
  ) {
    throw new TypeError('analysis config differs from the frozen manifest');
  }
  if (config.bootstrapSeed !== config.powerParameters.numericSeed) {
    throw new TypeError(
      'analysis bootstrap seed differs from frozen power seed',
    );
  }
  const expectedModel =
    config.family === 'replication'
      ? freeze.replication.candidate
      : freeze.primaryModel;
  if (
    config.provider !== expectedModel.provider ||
    config.model !== expectedModel.model ||
    config.effort !== expectedModel.effort
  ) {
    throw new TypeError('analysis model differs from the frozen family');
  }
  if (
    config.contrasts !== undefined &&
    (config.family !== 'secondary' ||
      config.contrasts.some(
        ({ reference }) => reference !== freeze.selectedBaseline,
      ) ||
      new Set(config.contrasts.map(({ id }) => id)).size !==
        config.contrasts.length)
  ) {
    throw new TypeError('analysis contrasts differ from the registered family');
  }
  if (
    config.family === 'sensitivity' &&
    config.modelCallBudgetP95 !== freeze.modelCallBudgetP95
  ) {
    throw new TypeError('analysis cap differs from the frozen p95 budget');
  }
  if (
    config.family !== 'sensitivity' &&
    config.modelCallBudgetP95 !== undefined
  ) {
    throw new TypeError('uncapped analysis cannot declare a model-call budget');
  }
  return config;
};

export const analyze = async (invocation: CliInvocation): Promise<unknown> => {
  rejectUnknownFlags(invocation, [
    'scores',
    'config',
    'freeze',
    'image',
    'work-dir',
    'result',
    'report',
  ]);
  const scores = requiredFlag(invocation, 'scores');
  const configPath = requiredFlag(invocation, 'config');
  const freeze = await loadFreezeManifest(requiredFlag(invocation, 'freeze'));
  if (freeze === undefined) throw new TypeError('freeze is required');
  const config = validateConfig(await readJson(configPath), freeze);
  if (!/^\/input\/config\/[^/]+$/u.test(config.powerResultPath)) {
    throw new TypeError(
      'powerResultPath must name a file in the mounted config directory',
    );
  }
  const powerResultPath = join(
    dirname(configPath),
    basename(config.powerResultPath),
  );
  const powerResult = PowerResultV1.parse(await readJson(powerResultPath));
  if (
    powerResult.configHash !== freeze.artifactHashes.powerConfig ||
    (await fileHash(powerResultPath)) !== freeze.artifactHashes.powerResult ||
    powerResult.nPower !== freeze.nPower ||
    powerResult.nFinal !== freeze.nFinal ||
    powerResult.minimumEffect !== freeze.minimumEffect ||
    powerResult.seed !== config.powerParameters.seed ||
    powerResult.numericSeed !== config.powerParameters.numericSeed ||
    powerResult.baselineProbability !==
      config.powerParameters.baselineProbability ||
    powerResult.randomInterceptSd !==
      config.powerParameters.randomInterceptSd ||
    artifactHash(powerResult.adaptiveClasses) !==
      artifactHash(config.powerParameters.adaptiveClasses)
  ) {
    throw new TypeError('analysis power result differs from the freeze');
  }
  const image = requiredFlag(invocation, 'image');
  if (imageDigest(image) !== freeze.containerDigest) {
    throw new TypeError('analysis image differs from the frozen OCI digest');
  }
  const destination = requiredFlag(invocation, 'result');
  if (await immutableExists(destination)) {
    throw new Error('analysis result already exists');
  }
  const reportPath = optionalFlag(invocation, 'report');
  if (reportPath !== undefined && (await immutableExists(reportPath))) {
    throw new Error('analysis report already exists');
  }
  progress('running frozen confirmatory analysis and report twice');
  const verificationDirectory = await runContainerVerifier(
    'verify-determinism.sh',
    [image, scores, configPath, requiredFlag(invocation, 'work-dir')],
  );
  const value = AnalysisResultV1.parse(
    await readAnalysisResult(join(verificationDirectory, 'result-1.json')),
  );
  const result = await writeJsonExclusive(destination, value);
  const report =
    reportPath === undefined
      ? undefined
      : await writeTextExclusive(
          reportPath,
          await readFile(join(verificationDirectory, 'report-1.md'), 'utf8'),
        );
  const checksums = await readFile(
    join(verificationDirectory, 'SHA256SUMS'),
    'utf8',
  );
  return {
    result,
    ...(report === undefined ? {} : { report }),
    verificationDirectory,
    checksumsHash: artifactHash(checksums),
  };
};
