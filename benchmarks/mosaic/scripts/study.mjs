#!/usr/bin/env node

import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  analyzeFamilies,
  createFamilySchedules,
  executeFamilies,
  writeFinalResult,
} from './study-confirmatory.mjs';
import {
  calibration,
  freeze,
  pilot,
  power,
  prepareFreeze,
  validateAuthoredInputs,
} from './study-preparation.mjs';
import { preflight } from './study-preflight.mjs';
import { setupIndex } from './study-setup.mjs';
import {
  loadApi,
  parseArgs,
  readJson,
  templateFor,
  usage,
  validateConfig,
  writeJson,
} from './study-runtime.mjs';
import { materializeInputs, pathsFor } from './study-inputs.mjs';
import { checkReadiness } from './study-readiness.mjs';

const validationResult = (config, stage) => ({
  schemaVersion: 1,
  ok: true,
  envelopeValid: true,
  stage,
  studyId: config.studyId,
  root: resolve(config.root),
  inputs: Object.keys(config.files).sort(),
});

const paidContext = async (configPath, stage) => {
  const validated = await validateConfig(await readJson(configPath), stage);
  const paths = pathsFor(resolve(validated.root));
  const config = await materializeInputs(validated, configPath, paths, stage);
  await Promise.all(
    Object.values(paths).map((path) => mkdir(path, { recursive: true })),
  );
  return {
    config,
    paths,
    api: undefined,
    commit: undefined,
    cases: undefined,
  };
};

export const runStudy = async (argv = process.argv.slice(2)) => {
  const args = parseArgs(argv);
  if (args.help) return process.stdout.write(`${usage}\n`);
  if (args.print) {
    return process.stdout.write(
      `${JSON.stringify(templateFor(args.stage ?? 'prepare'), null, 2)}\n`,
    );
  }
  if (args.validate !== undefined) {
    const config = await validateConfig(
      await readJson(resolve(args.validate)),
      args.stage,
    );
    return process.stdout.write(
      `${JSON.stringify(validationResult(config, args.stage))}\n`,
    );
  }
  if (args.readiness !== undefined) {
    const result = await checkReadiness(resolve(args.readiness), args.stage);
    return process.stdout.write(`${JSON.stringify(result)}\n`);
  }
  if (!args.paid) {
    throw new Error('refusing paid execution without --yes-paid-study');
  }
  const stage = args.prepare !== undefined ? 'prepare' : 'continue';
  const configArgument = args.prepare ?? args.continue;
  if (configArgument === undefined) {
    throw new Error('--prepare or --continue is required');
  }
  if (!process.env.OPENROUTER_API_KEY?.trim()) {
    throw new Error('OPENROUTER_API_KEY is required');
  }

  const configPath = resolve(configArgument);
  const context = await paidContext(configPath, stage);
  await preflight(context);
  context.api = await loadApi();
  await validateAuthoredInputs(context);
  await setupIndex(context);
  if (stage === 'prepare') {
    await pilot(context);
    await calibration(context);
    const powerResult = await power(context);
    const result = {
      schemaVersion: 1,
      studyId: context.config.studyId,
      stage: 'power',
      gitCommit: context.commit,
      nPower: powerResult.nPower,
      nFinal: powerResult.nFinal,
      powerResultHash: context.api.core.artifactHash(powerResult),
      next: 'author and audit exactly nFinal confirmatory cases, then run --continue',
    };
    await writeJson(
      context,
      resolve(context.paths.power, 'preparation.json'),
      result,
    );
    return process.stdout.write(`${context.api.core.canonicalJson(result)}\n`);
  }
  const preparation = await readJson(
    resolve(context.paths.power, 'preparation.json'),
  );
  const powerResult = context.api.PowerResultV1.parse(
    await readJson(resolve(context.paths.power, 'power-result.json')),
  );
  if (
    preparation.studyId !== context.config.studyId ||
    preparation.gitCommit !== context.commit ||
    preparation.nFinal !== powerResult.nFinal ||
    preparation.powerResultHash !== context.api.core.artifactHash(powerResult)
  ) {
    throw new Error('continue does not match the immutable preparation result');
  }
  const prepared = await prepareFreeze(context, powerResult);
  const manifest = await freeze(context, prepared);
  await createFamilySchedules(context, prepared, manifest);
  await executeFamilies(context);
  await analyzeFamilies(context, manifest, powerResult);
  const result = await writeFinalResult(context, manifest);
  process.stdout.write(`${context.api.core.canonicalJson(result)}\n`);
};

const invoked =
  process.argv[1] !== undefined &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (invoked) {
  runStudy().catch((error) => {
    const message = error instanceof Error ? error.message : 'failed';
    process.stdout.write(
      `${JSON.stringify({
        schemaVersion: 1,
        ok: false,
        error: { code: 'study_failed', message },
      })}\n`,
    );
    process.stderr.write(`study: ${message}\n`);
    process.exitCode = 1;
  });
}
