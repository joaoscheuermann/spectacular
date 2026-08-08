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
  preflight,
  prepareFreeze,
  validateAuthoredInputs,
} from './study-preparation.mjs';
import {
  loadApi,
  materializeInputs,
  parseArgs,
  pathsFor,
  readJson,
  template,
  usage,
  validateConfig,
} from './study-runtime.mjs';

const validationResult = (config) => ({
  schemaVersion: 1,
  ok: true,
  studyId: config.studyId,
  root: resolve(config.root),
  inputs: Object.keys(config.files).sort(),
});

const paidContext = async (configPath) => {
  const validated = await validateConfig(await readJson(configPath));
  const paths = pathsFor(resolve(validated.root));
  const config = await materializeInputs(validated, configPath, paths);
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
    return process.stdout.write(`${JSON.stringify(template, null, 2)}\n`);
  }
  if (args.validate !== undefined) {
    const config = await validateConfig(await readJson(resolve(args.validate)));
    return process.stdout.write(
      `${JSON.stringify(validationResult(config))}\n`,
    );
  }
  if (!args.paid) {
    throw new Error('refusing paid execution without --yes-paid-study');
  }
  if (!args.config) throw new Error('--config is required');
  if (!process.env.OPENROUTER_API_KEY?.trim()) {
    throw new Error('OPENROUTER_API_KEY is required');
  }

  const configPath = resolve(args.config);
  const context = await paidContext(configPath);
  await preflight(context);
  context.api = await loadApi();
  await validateAuthoredInputs(context);
  await pilot(context);
  await calibration(context);
  const powerResult = await power(context);
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
