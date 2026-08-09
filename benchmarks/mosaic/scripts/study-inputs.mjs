import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';

import { exists, fail } from './study-runtime.mjs';

export const pathsFor = (root) => ({
  inputs: join(root, 'inputs'),
  preflight: join(root, 'preflight'),
  pilot: join(root, 'pilot'),
  calibration: join(root, 'calibration'),
  power: join(root, 'power'),
  freeze: join(root, 'freeze'),
  schedules: join(root, 'schedules'),
  primary: join(root, 'primary'),
  replication: join(root, 'replication'),
  sensitivity: join(root, 'sensitivity'),
  analysisConfig: join(root, 'analysis', 'config'),
  final: join(root, 'final'),
});

const copyExact = async (source, destination) => {
  const bytes = await readFile(source);
  if (await exists(destination)) {
    if (!bytes.equals(await readFile(destination))) {
      fail(`immutable study input differs: ${destination}`);
    }
    return;
  }
  await mkdir(dirname(destination), { recursive: true });
  await writeFile(destination, bytes, { flag: 'wx', mode: 0o600 });
};

const studyIdentity = (config) =>
  `${JSON.stringify(
    {
      schemaVersion: config.schemaVersion,
      studyId: config.studyId,
      root: config.root,
      analysisImage: config.analysisImage,
      frozenAt: config.frozenAt,
      candidate: config.candidate,
      seeds: config.seeds,
    },
    null,
    2,
  )}\n`;

/** Copies every scientific input once so it cannot drift during long runs. */
export const materializeInputs = async (
  config,
  configPath,
  paths,
  stage = 'continue',
) => {
  const names = {
    prices: 'prices.json',
    calibrationCases: 'calibration-cases.json',
    calibrationAudit: 'calibration-audit.json',
    confirmatoryCases: 'confirmatory-cases.json',
    confirmatoryAudit: 'confirmatory-audit.json',
    costApproval: 'cost-approval.json',
    powerConfig: 'power-config.json',
    powerApproval: 'power-approval.json',
  };
  const destinations = Object.fromEntries(
    Object.keys(config.files).map((name) => [
      name,
      join(paths.inputs, names[name]),
    ]),
  );
  const identity = studyIdentity(config);
  const identityPath = join(paths.inputs, 'study.identity.json');
  if (await exists(identityPath)) {
    if ((await readFile(identityPath, 'utf8')) !== identity) {
      fail('study identity changed; use a new study root');
    }
  } else {
    await mkdir(dirname(identityPath), { recursive: true });
    await writeFile(identityPath, identity, { flag: 'wx', mode: 0o600 });
  }
  await copyExact(configPath, join(paths.inputs, `study.${stage}.json`));
  await Promise.all(
    Object.entries(config.files).map(([name, source]) =>
      copyExact(source, destinations[name]),
    ),
  );
  return { ...config, files: destinations };
};
