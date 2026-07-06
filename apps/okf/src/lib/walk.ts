import fs from 'node:fs';
import path from 'node:path';

import { glob } from 'glob';
import { readGitIgnoreFile } from './git.js';

interface WalkOptions {
  ignore?: Array<string>;
}

/** walks the file system ignoring files for the current root path */
export async function walk(
  root: string,
  callback: (file: string, body: string) => Promise<void>,
  options: WalkOptions,
): Promise<void> {
  const ignore = [
    ...(options.ignore ?? []),
    ...(await readGitIgnoreFile(root)),
  ];

  const targets = await glob('*', {
    cwd: root,
    ignore,
  });

  for (const target of targets) {
    const composed = path.join(root, target);
    const stat = await fs.promises.stat(composed);

    if (stat.isDirectory()) {
      await walk(composed, callback, { ignore });

      // Handle the folder...
    } else {
      const body = await fs.promises.readFile(composed, 'utf-8');

      await callback(composed, body);
    }
  }
}
