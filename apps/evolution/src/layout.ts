import { randomUUID } from 'node:crypto';
import { lstat, mkdir, rename, writeFile } from 'node:fs/promises';
import { dirname, isAbsolute, join, relative } from 'node:path';

import type { TargetResult } from './evolve.js';

export type Layout = {
  readonly root: string;
  readonly targets: readonly TargetResult[];
  readonly dryRun: boolean;
};

const atomicWrite = async (path: string, body: string): Promise<void> => {
  await mkdir(dirname(path), { recursive: true });
  const temporary = path + '.' + randomUUID() + '.tmp';
  await writeFile(temporary, body, 'utf8');
  await rename(temporary, path);
};

/** Rejects writes outside the workspace or through existing symlink components. */
export const assertSafeWritePath = async (
  root: string,
  path: string,
): Promise<void> => {
  const child = relative(root, path);
  if (child.startsWith('..') || isAbsolute(child)) {
    throw new Error('Write path escapes the evolution root: ' + path);
  }
  const rootStats = await lstat(root);
  if (rootStats.isSymbolicLink()) {
    throw new Error('Write path contains a symbolic link or junction: ' + root);
  }
  let current = root;
  for (const component of child.split(/[\\/]/u).filter(Boolean)) {
    current = join(current, component);
    try {
      const stats = await lstat(current);
      if (stats.isSymbolicLink()) {
        throw new Error(
          'Write path contains a symbolic link or junction: ' + current,
        );
      }
    } catch (error) {
      if (
        error instanceof Error &&
        'code' in error &&
        error.code === 'ENOENT'
      ) {
        return;
      }
      throw error;
    }
  }
};

/** Preflights every approved prompt destination without mutating it. */
export const preflightLayout = async (layout: Layout): Promise<void> => {
  const approved = layout.targets.filter(({ approved }) => approved);
  const paths = approved.map(({ target }) =>
    join(layout.root, target.id, 'SYSTEM_PROMPT.md'),
  );
  await Promise.all(
    paths.map((path) => assertSafeWritePath(layout.root, path)),
  );
};

/** Writes prompts only for targets approved by held-out validation. */
export const persistLayout = async (layout: Layout): Promise<void> => {
  if (layout.dryRun) return;
  await preflightLayout(layout);
  const approved = layout.targets.filter(({ approved }) => approved);
  await Promise.all(
    approved.map(({ target, prompt }) =>
      atomicWrite(
        join(layout.root, target.id, 'SYSTEM_PROMPT.md'),
        prompt.trimEnd() + '\n',
      ),
    ),
  );
};
