import fs from 'node:fs';
import path from 'node:path';

import { glob } from 'glob';
import { readGitIgnoreFile } from './git.js';

export type WalkFile = {
  readonly path: string;
  readonly body: string;
  readonly output: string;
};

export type WalkCallbacks = {
  readonly file: (file: string, body: string) => Promise<string>;
  readonly folder: (
    root: string,
    files: readonly WalkFile[],
  ) => Promise<void>;
};

export type WalkOptions = {
  readonly ignore?: readonly string[];
};

/** Walks the file system from root while respecting .gitignore and explicit ignores. */
export async function walk(
  root: string,
  callbacks: WalkCallbacks,
  options: WalkOptions,
): Promise<readonly WalkFile[]> {
  const ignore = [
    ...(options.ignore ?? []),
    ...(await readGitIgnoreFile(root)),
  ];
  const files: WalkFile[] = [];

  const targets = await glob('*', {
    cwd: root,
    ignore,
  });

  for (const target of targets) {
    const composed = path.join(root, target);
    const stat = await fs.promises.stat(composed);

    if (stat.isDirectory()) {
      files.push(...(await walk(composed, callbacks, { ignore })));

      continue;
    } else {
      const body = await fs.promises.readFile(composed, 'utf-8');
      const output = await callbacks.file(composed, body);

      files.push({
        path: composed,
        body,
        output,
      });
    }
  }

  await callbacks.folder(root, files);

  return files;
}
