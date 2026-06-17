import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import { createTool } from '../src/index.js';

describe('edit tool', () => {
  test('updates a unique exact match and returns first changed line', async () => {
    const root = await workspace('edit-exact');
    await write(root, 'src/lib.ts', 'one\noldCall();\nthree\n');

    const result = await createTool({ workspaceRoot: root }).execute({
      path: 'src/lib.ts',
      edits: [{ oldText: 'oldCall();', newText: 'newCall();' }],
    });

    assert.equal(result.success, true);
    assert.equal(result.first_changed_line, 2);
    assert.match(result.diff, /2 -oldCall\(\);/);
    assert.match(result.diff, /2 \+newCall\(\);/);
    assert.equal(
      await readFile(path.join(root, 'src/lib.ts'), 'utf8'),
      'one\nnewCall();\nthree\n',
    );
    await rm(root, { recursive: true, force: true });
  });

  test('rejects duplicate old text with context guidance', async () => {
    const root = await workspace('edit-duplicate');
    await write(root, 'same.txt', 'same\nsame\n');

    const result = await createTool({ workspaceRoot: root }).execute({
      path: 'same.txt',
      edits: [{ old_text: 'same', new_text: 'changed' }],
    });

    assert.equal(result.success, false);
    assert.match(result.error ?? '', /Please provide more context/);
    await rm(root, { recursive: true, force: true });
  });

  test('distinguishes missing files from other read failures', async () => {
    const root = await workspace('edit-read-errors');
    await mkdir(path.join(root, 'src'), { recursive: true });
    const tool = createTool({ workspaceRoot: root });

    const missing = await tool.execute({
      path: 'missing.txt',
      edits: [{ oldText: 'old', newText: 'new' }],
    });
    const directory = await tool.execute({
      path: 'src',
      edits: [{ oldText: 'old', newText: 'new' }],
    });

    assert.match(missing.error ?? '', /^File not found: missing\.txt$/);
    assert.match(directory.error ?? '', /^Failed to read file:/);
    await rm(root, { recursive: true, force: true });
  });
});

const workspace = (name: string): Promise<string> =>
  mkdtemp(path.join(os.tmpdir(), `doric-${name}-`));

const write = async (
  root: string,
  file: string,
  content: string,
): Promise<void> => {
  const target = path.join(root, file);
  await mkdir(path.dirname(target), { recursive: true });
  await writeFile(target, content, 'utf8');
};
