/*
Prepare a run from CLI options and resolved configuration without provider or Docker calls.
Load the request, local catalog and compiled core bundle, reject duplicate skill names,
prepare local inputs and persist a UUID manifest with source hashes including all stages.
Return the output directory, manifest and core bundle for sandbox execution.
**/

import { createHash, randomUUID } from 'node:crypto';
import {
  cp,
  mkdir,
  readdir,
  readFile,
  stat,
  writeFile,
} from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { loadBundles } from 'bundle';

const directory = resolve(dirname(fileURLToPath(import.meta.url)), '..');

const loadRequest = async (path) => {
  const request = (await readFile(path, 'utf8')).trim();

  if (!request) {
    throw new Error('Request is empty.');
  }

  return request;
};

const loadSkills = async (path) => {
  const entries = await readdir(path);
  const filenames = entries.filter((name) => name.endsWith('.md')).sort();
  const skills = [];

  for (const filename of filenames) {
    const body = (await readFile(join(path, filename), 'utf8')).trim();

    skills.push({ name: basename(filename, '.md'), body });
  }

  if (skills.length === 0 || skills.some(({ body }) => !body)) {
    throw new Error('Provide nonempty Markdown skills.');
  }

  return skills;
};

const sourceHashes = async () => {
  const entries = await readdir(directory);
  const stageEntries = await readdir(join(directory, 'stages'));

  const filenames = [
    ...entries.filter((name) => name.endsWith('.mjs')),
    ...stageEntries
      .filter((name) => name.endsWith('.mjs'))
      .map((name) => 'stages/' + name),
  ].sort();
  const hashes = [];

  for (const name of filenames) {
    const content = await readFile(join(directory, name));
    const sha256 = createHash('sha256').update(content).digest('hex');

    hashes.push({ name, sha256 });
  }

  return hashes;
};

const prepareWorkspace = async (output, existingWorkspace) => {
  if (existingWorkspace) {
    const workspace = resolve(existingWorkspace);

    if (!(await stat(workspace)).isDirectory()) {
      throw new Error('Workspace must be a directory.');
    }

    await mkdir(output, { recursive: true });

    return workspace;
  }

  const workspace = join(output, 'workspace');

  await mkdir(workspace, { recursive: true });

  await cp(join(directory, 'case/input.csv'), join(workspace, 'input.csv'));

  return workspace;
};

const saveJson = (path, value) =>
  writeFile(path, JSON.stringify(value, null, 2) + '\n');

export const prepareRun = async ({ options, config }) => {
  if (Boolean(options.request) !== Boolean(options.workspace)) {
    throw new Error('Provide --request and --workspace together.');
  }

  const requestPath = options.request ?? join(directory, 'case/request.md');
  const request = await loadRequest(requestPath);

  const bundles = await loadBundles(
    join(directory, '../../agents/doric/dist/bundles'),
  );
  const core = bundles.find(({ name }) => name === 'core');

  if (!core) {
    throw new Error('Build the core bundle: npx nx run bundle-core:build');
  }

  const skills = [
    ...(await loadSkills(options.skills)),
    ...core.skills
      .filter(({ alwaysAvailable }) => !alwaysAvailable)
      .map(({ skill }) => skill),
  ];

  const names = [
    ...skills,
    ...core.skills
      .filter(({ alwaysAvailable }) => alwaysAvailable)
      .map(({ skill }) => skill),
  ].map(({ name }) => name);

  if (new Set(names).size !== names.length) {
    throw new Error('Duplicate skill names in the combined catalog.');
  }

  const id = randomUUID();
  const output = join(directory, 'output', id);
  const workspace = await prepareWorkspace(output, options.workspace);

  const manifest = {
    id,
    startedAt: new Date().toISOString(),
    config,
    workspace,
    request,
    skills,
    core: {
      skills: core.skills,
      tools: core.tools.map(({ factory }) => factory.definition),
    },
    sources: await sourceHashes(),
    nodeVersion: process.version,
    status: options.prepare ? 'prepared' : 'running',
  };

  await saveJson(join(output, 'manifest.json'), manifest);

  console.log(
    'Run: ' + id + '\nOutput: ' + output + '\nWorkspace: ' + workspace,
  );

  return { output, manifest, core };
};
