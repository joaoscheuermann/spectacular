import { Glob } from 'glob';
import type { Ignore } from 'ignore';
import { lstat, readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';

import type { CollectedEntry } from './types.js';
import {
  isAgentsPath,
  isDoricPath,
  isGitPath,
  normalizeRelativePath,
} from './paths.js';

const require = createRequire(import.meta.url);
const createIgnore = require('ignore') as () => Ignore;

type ScopedIgnore = {
  readonly basePath: string;
  readonly matcher: Ignore;
};

/** Collects the target repository structure with glob.walk and .gitignore rules. */
export const collectStructure = async (
  rootPath: string,
): Promise<readonly CollectedEntry[]> => {
  const root = path.resolve(rootPath);
  const globber = new Glob('**/*', {
    cwd: root,
    dot: true,
    ignore: ['.agents/**', '.doric/**', '.git/**'],
    nodir: false,
    windowsPathsNoEscape: true,
  });
  const matches = await globber.walk();
  const entries = await Promise.all(
    matches.map((match) => entryFromMatch(root, String(match))),
  );
  const collected = entries.filter(
    (entry): entry is CollectedEntry => entry !== undefined,
  );
  const matchers = await ignoreMatchers(root, collected);

  return collected
    .filter((entry) => !isIgnored(matchers, entry))
    .sort((first, second) =>
      first.relativePath.localeCompare(second.relativePath),
    );
};

const entryFromMatch = async (
  rootPath: string,
  match: string,
): Promise<CollectedEntry | undefined> => {
  const relativePath = normalizeRelativePath(match);

  if (
    relativePath === '' ||
    isAgentsPath(relativePath) ||
    isDoricPath(relativePath) ||
    isGitPath(relativePath)
  ) {
    return undefined;
  }

  const absolutePath = path.join(rootPath, ...relativePath.split('/'));

  try {
    const stats = await lstat(absolutePath);

    if (stats.isDirectory()) {
      return { type: 'directory', absolutePath, relativePath };
    }

    if (stats.isFile()) {
      return { type: 'file', absolutePath, relativePath };
    }

    return undefined;
  } catch {
    return undefined;
  }
};

const ignoreMatchers = async (
  rootPath: string,
  entries: readonly CollectedEntry[],
): Promise<readonly ScopedIgnore[]> => {
  const gitignoreFiles = entries
    .filter((entry) => entry.type === 'file')
    .filter((entry) => path.posix.basename(entry.relativePath) === '.gitignore')
    .sort(
      (first, second) =>
        depth(first.relativePath) - depth(second.relativePath) ||
        first.relativePath.localeCompare(second.relativePath),
    );
  const matchers = await Promise.all(
    gitignoreFiles.map((entry) => scopedIgnore(rootPath, entry.relativePath)),
  );

  return matchers.filter(
    (matcher): matcher is ScopedIgnore => matcher !== undefined,
  );
};

const scopedIgnore = async (
  rootPath: string,
  relativePath: string,
): Promise<ScopedIgnore | undefined> => {
  try {
    const matcher = createIgnore();
    matcher.add(
      await readFile(path.join(rootPath, ...relativePath.split('/')), 'utf8'),
    );

    return {
      basePath:
        path.posix.dirname(relativePath) === '.'
          ? ''
          : path.posix.dirname(relativePath),
      matcher,
    };
  } catch {
    return undefined;
  }
};

const isIgnored = (
  matchers: readonly ScopedIgnore[],
  entry: CollectedEntry,
): boolean => {
  if (
    isAgentsPath(entry.relativePath) ||
    isDoricPath(entry.relativePath) ||
    isGitPath(entry.relativePath)
  ) {
    return true;
  }

  let ignored = false;

  for (const scoped of matchers) {
    const pathname = pathInScope(entry, scoped.basePath);

    if (pathname === undefined) {
      continue;
    }

    const result = scoped.matcher.test(pathname);

    if (result.ignored) {
      ignored = true;
    }

    if (result.unignored) {
      ignored = false;
    }
  }

  return ignored;
};

const pathInScope = (
  entry: CollectedEntry,
  basePath: string,
): string | undefined => {
  const relativePath = normalizeRelativePath(entry.relativePath);

  if (basePath !== '') {
    if (relativePath !== basePath && !relativePath.startsWith(`${basePath}/`)) {
      return undefined;
    }
  }

  const localPath =
    basePath === ''
      ? relativePath
      : normalizeRelativePath(relativePath.slice(basePath.length + 1));

  if (localPath === '') {
    return undefined;
  }

  return entry.type === 'directory' ? `${localPath}/` : localPath;
};

const depth = (relativePath: string): number => {
  const normalized = normalizeRelativePath(relativePath);

  return normalized === '' ? 0 : normalized.split('/').length;
};
