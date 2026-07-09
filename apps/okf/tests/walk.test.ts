import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';

import { walk, type WalkFile } from '../src/lib/walk.js';

type SeenFile = {
  readonly path: string;
  readonly body: string;
  readonly output: string;
};

type FolderCall = {
  readonly root: string;
  readonly files: readonly SeenFile[];
};

const normalize = (
  root: string,
  files: readonly WalkFile[],
): readonly SeenFile[] =>
  files
    .map((file) => ({
      path: toPosix(path.relative(root, file.path)),
      body: file.body,
      output: file.output,
    }))
    .sort((left, right) => left.path.localeCompare(right.path));

const normalizeFolder = (root: string, folder: string): string => {
  const relative = path.relative(root, folder);

  return relative === '' ? '.' : toPosix(relative);
};

const toPosix = (value: string): string => value.split(path.sep).join('/');

test('passes file outputs to folder callbacks for each subtree', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'okf-walk-'));

  try {
    await fs.mkdir(path.join(root, 'docs', 'nested'), { recursive: true });
    await Promise.all([
      fs.writeFile(path.join(root, 'root.txt'), 'root', 'utf-8'),
      fs.writeFile(path.join(root, 'docs', 'guide.md'), 'guide', 'utf-8'),
      fs.writeFile(
        path.join(root, 'docs', 'nested', 'detail.txt'),
        'detail',
        'utf-8',
      ),
    ]);

    const folderCalls: FolderCall[] = [];

    const files = await walk(
      root,
      {
        file: async (file, body) =>
          `out:${toPosix(path.relative(root, file))}:${body.length}`,
        folder: async (folderRoot, records) => {
          folderCalls.push({
            root: normalizeFolder(root, folderRoot),
            files: normalize(root, records),
          });
        },
      },
      { ignore: [] },
    );

    const rootFiles = [
      {
        path: 'docs/guide.md',
        body: 'guide',
        output: 'out:docs/guide.md:5',
      },
      {
        path: 'docs/nested/detail.txt',
        body: 'detail',
        output: 'out:docs/nested/detail.txt:6',
      },
      {
        path: 'root.txt',
        body: 'root',
        output: 'out:root.txt:4',
      },
    ];

    assert.deepEqual(normalize(root, files), rootFiles);
    assert.equal(folderCalls.length, 3);

    const folders = new Map(
      folderCalls.map((folder) => [folder.root, folder.files]),
    );

    assert.deepEqual(Array.from(folders.keys()).sort(), [
      '.',
      'docs',
      'docs/nested',
    ]);
    assert.deepEqual(folders.get('docs/nested'), [
      {
        path: 'docs/nested/detail.txt',
        body: 'detail',
        output: 'out:docs/nested/detail.txt:6',
      },
    ]);
    assert.deepEqual(folders.get('docs'), [
      {
        path: 'docs/guide.md',
        body: 'guide',
        output: 'out:docs/guide.md:5',
      },
      {
        path: 'docs/nested/detail.txt',
        body: 'detail',
        output: 'out:docs/nested/detail.txt:6',
      },
    ]);
    assert.deepEqual(folders.get('.'), rootFiles);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
