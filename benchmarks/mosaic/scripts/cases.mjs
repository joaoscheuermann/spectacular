#!/usr/bin/env node

import { open, readFile } from 'node:fs/promises';

const parse = (argv) => {
  const [command, ...tail] = argv;
  if (command !== 'compile' && command !== 'validate') {
    throw new TypeError('command must be compile or validate');
  }
  const flags = {};
  for (let index = 0; index < tail.length; index += 2) {
    const name = tail[index];
    const value = tail[index + 1];
    if (
      !name?.startsWith('--') ||
      value === undefined ||
      value.startsWith('--')
    ) {
      throw new TypeError(`invalid option: ${name ?? ''}`);
    }
    const key = name.slice(2);
    if (key in flags) throw new TypeError(`duplicate option: ${name}`);
    flags[key] = value;
  }
  const allowed =
    command === 'compile'
      ? new Set(['draft', 'output', 'confirmatory-cases'])
      : new Set(['calibration-cases', 'confirmatory-cases']);
  const unknown = Object.keys(flags).find((key) => !allowed.has(key));
  if (unknown !== undefined)
    throw new TypeError(`unknown option: --${unknown}`);
  return { command, flags };
};

const required = (flags, name) => {
  const value = flags[name];
  if (value === undefined) throw new TypeError(`--${name} is required`);
  return value;
};

const readJson = async (path) => JSON.parse(await readFile(path, 'utf8'));

const writeExclusive = async (path, content) => {
  const handle = await open(path, 'wx', 0o600);
  try {
    await handle.writeFile(content, 'utf8');
    await handle.sync();
  } finally {
    await handle.close();
  }
};

const loadConfirmatory = async (api, path) => {
  if (path === undefined) return [];
  const value = await readJson(path);
  if (!Array.isArray(value))
    throw new TypeError('confirmatory cases must be an array');
  return value.map((entry) => api.CaseV1.parse(entry));
};

const summary = (report, extra = {}) => ({
  schemaVersion: 1,
  valid: report.valid,
  caseCount: report.cases.length,
  issues: report.issues,
  humanAuditRequired: report.humanAuditRequired,
  ...extra,
});

const main = async () => {
  const invocation = parse(process.argv.slice(2));
  const api = await import('../dist/src/index.js');
  const confirmatory = await loadConfirmatory(
    api,
    invocation.flags['confirmatory-cases'],
  );
  if (invocation.command === 'compile') {
    const output = required(invocation.flags, 'output');
    const report = api.study.compileCalibrationCases(
      await readJson(required(invocation.flags, 'draft')),
      api.study.PILOT_CASES,
      confirmatory,
    );
    if (report.valid) {
      await writeExclusive(output, `${api.core.canonicalJson(report.cases)}\n`);
    }
    process.stdout.write(
      `${api.core.canonicalJson(
        summary(report, {
          written: report.valid,
          outputHash: report.valid ? api.core.artifactHash(report.cases) : null,
        }),
      )}\n`,
    );
    if (!report.valid) process.exitCode = 1;
    return;
  }
  const report = api.study.validateCalibrationCorpus(
    await readJson(required(invocation.flags, 'calibration-cases')),
    api.study.PILOT_CASES,
    confirmatory,
  );
  process.stdout.write(`${api.core.canonicalJson(summary(report))}\n`);
  if (!report.valid) process.exitCode = 1;
};

main().catch((error) => {
  const existing =
    typeof error === 'object' && error !== null && error.code === 'EEXIST';
  process.stdout.write(
    `${JSON.stringify({
      schemaVersion: 1,
      valid: false,
      issues: [
        {
          code: existing ? 'output_exists' : 'command_failed',
          caseId: '$',
          detail: existing
            ? 'refusing to overwrite existing output'
            : error instanceof Error
              ? error.message
              : 'case command failed',
        },
      ],
      humanAuditRequired: ['semantic-neutrality', 'cross-phase-independence'],
    })}\n`,
  );
  process.exitCode = 1;
});
