import { access, mkdtemp, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { renderArtifact, saveArtifact, type Artifact } from '../src/index.js';

test('renders artifact data through the artifact template', () => {
  const artifact: Artifact<{ readonly title: string }> = {
    name: 'SUMMARY',
    mime: 'text/markdown',
    data: { title: 'Doric' },
    template: (data) => `# ${data.title}`,
  };

  assert.equal(renderArtifact(artifact), '# Doric');
});

test('saves markdown artifacts with a default md filename derived from the artifact name', async () => {
  const root = await workspace('artifact-markdown');
  const artifact = markdown('PROMPT', '# Prompt');

  const written = await saveArtifact(artifact, { directory: root });

  assert.equal(path.basename(written), 'PROMPT.md');
  assert.equal(await readFile(written, 'utf8'), '# Prompt');
  await rm(root, { recursive: true, force: true });
});

test('creates missing output directories before writing artifacts', async () => {
  const root = await workspace('artifact-directories');
  const directory = path.join(root, 'nested', 'artifacts');

  const written = await saveArtifact(markdown('PROMPT', 'content'), {
    directory,
  });

  await access(written);
  assert.equal(await readFile(written, 'utf8'), 'content');
  await rm(root, { recursive: true, force: true });
});

test('rejects unsafe artifact names before deriving default filenames', async () => {
  const root = await workspace('artifact-unsafe-name');
  const unsafeNames = [
    '',
    ' ',
    '.',
    '..',
    '../PROMPT',
    'nested/PROMPT',
    'a..b',
  ];

  for (const name of unsafeNames) {
    await assert.rejects(
      saveArtifact(markdown(name, 'content'), { directory: root }),
      /Unsafe artifact name/,
    );
  }

  await rm(root, { recursive: true, force: true });
});

test('rejects unsafe explicit filenames before writing artifacts', async () => {
  const root = await workspace('artifact-unsafe-filename');
  const unsafeFileNames = [
    '',
    '.',
    '..',
    '../PROMPT.md',
    'nested/PROMPT.md',
    'C:\\PROMPT.md',
    '/tmp/PROMPT.md',
    'PROMPT..md',
  ];

  for (const fileName of unsafeFileNames) {
    await assert.rejects(
      saveArtifact(markdown('PROMPT', 'content'), {
        directory: root,
        fileName,
      }),
      /Unsafe artifact file name/,
    );
  }

  await rm(root, { recursive: true, force: true });
});

const markdown = (name: string, content: string): Artifact<string> => ({
  name,
  mime: 'text/markdown',
  data: content,
  template: (data) => data,
});

const workspace = (name: string): Promise<string> =>
  mkdtemp(path.join(os.tmpdir(), `doric-${name}-`));
