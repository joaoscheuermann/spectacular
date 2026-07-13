import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { test } from 'node:test';

import { SYSTEM_PROMPT } from '../prompts/analyze/index.js';
import { KINDS } from '../src/lib/agents/classify/kinds.js';

test('selects the default system prompt with the curried API', async () => {
  const select = SYSTEM_PROMPT('default');

  const actual = await select('source_code');
  const expected = await fs.readFile(
    path.resolve(
      'packages/okf/prompts/analyze/source_code/default/SYSTEM_PROMPT.md',
    ),
    'utf-8',
  );

  assert.equal(actual, expected.trim());
});

test('provides a default prompt for every supported classification kind', async () => {
  const select = SYSTEM_PROMPT('default');
  const prompts = await Promise.all(KINDS.map(select));
  const analyzeRoot = path.resolve('packages/okf/prompts/analyze');
  const directories = (await fs.readdir(analyzeRoot, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();

  assert.equal(prompts.length, KINDS.length);
  assert.ok(prompts.every((prompt) => prompt.length > 0));
  assert.deepEqual(directories, [...KINDS].sort());
  await assert.rejects(
    fs.access(path.join(analyzeRoot, 'default', 'SYSTEM_PROMPT.md')),
    /ENOENT/u,
  );
});

test('rejects unsupported kinds and invalid or missing prompt targets', async () => {
  await assert.rejects(
    SYSTEM_PROMPT('default')('unknown'),
    /Unsupported OKF analysis kind: unknown/u,
  );
  await assert.rejects(
    SYSTEM_PROMPT('con')('source_code'),
    /Invalid OKF prompt target: con/u,
  );
  await assert.rejects(
    SYSTEM_PROMPT('missing')('source_code'),
    /Cannot load OKF analyze prompt for kind source_code target missing/u,
  );
});

test('loads a classified prompt through the built selector', async () => {
  const built = pathToFileURL(
    path.resolve('packages/okf/dist/prompts/analyze/index.js'),
  ).href;
  const module = (await import(built)) as {
    readonly SYSTEM_PROMPT: typeof SYSTEM_PROMPT;
  };

  const actual = await module.SYSTEM_PROMPT('default')('localization');
  const expected = await SYSTEM_PROMPT('default')('localization');

  assert.equal(actual, expected);
});
