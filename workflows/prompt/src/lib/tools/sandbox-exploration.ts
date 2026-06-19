import { posix as path } from 'node:path';

import type { SandboxSession } from 'sandbox';

import {
  isForbiddenAbsolutePath,
  isIgnored,
  type IgnorePattern,
} from './safety.js';

export type ExplorationOptions = {
  readonly workspaceRoot: string;
  readonly sandbox: SandboxSession;
};

export type PathKind = 'directory' | 'file' | 'missing' | 'other';

export type Entry = {
  readonly path: string;
  readonly isDirectory: boolean;
};

const GUIDANCE_PRUNE_ARGS = [
  '(',
  '-name',
  '.git',
  '-o',
  '-name',
  'node_modules',
  '-o',
  '-name',
  'docs',
  '-o',
  '-name',
  '.agents',
  ')',
  '-prune',
  '-o',
] as const;

export const visibleFiles = async (
  options: ExplorationOptions,
  root: string,
): Promise<readonly string[]> => {
  const [files, ignores] = await Promise.all([
    listPaths(options, root, 'f'),
    readIgnores(options, root),
  ]);

  return files.filter(
    (file) =>
      !isForbiddenAbsolutePath(options.workspaceRoot, file) &&
      !isIgnoredWithAncestors(root, file, false, ignores),
  );
};

export const visibleEntries = async (
  options: ExplorationOptions,
  root: string,
  excludes: readonly RegExp[],
): Promise<readonly Entry[]> => {
  const [directories, files, ignores] = await Promise.all([
    listPaths(options, root, 'd'),
    listPaths(options, root, 'f'),
    readIgnores(options, root),
  ]);
  const entries = [
    ...directories
      .filter((entry) => entry !== root)
      .map((entry) => ({ path: entry, isDirectory: true })),
    ...files.map((entry) => ({ path: entry, isDirectory: false })),
  ];

  return entries.filter(
    (entry) =>
      !isForbiddenAbsolutePath(options.workspaceRoot, entry.path) &&
      !isHidden(root, entry.path) &&
      !isIgnoredWithAncestors(root, entry.path, entry.isDirectory, ignores) &&
      !isExcluded(entry.path, excludes),
  );
};

export const pathKind = async (
  { sandbox, workspaceRoot }: ExplorationOptions,
  value: string,
): Promise<PathKind> => {
  const result = await sandbox.exec({
    cwd: workspaceRoot,
    cmd: [
      'sh',
      '-c',
      'if [ -d "$1" ]; then printf directory; elif [ -f "$1" ]; then printf file; elif [ -e "$1" ]; then printf other; else printf missing; fi',
      'sh',
      value,
    ],
  });

  if (result.exitCode !== 0) {
    return 'missing';
  }

  return parsePathKind(result.stdout);
};

const listPaths = async (
  { sandbox, workspaceRoot }: ExplorationOptions,
  root: string,
  type: 'd' | 'f',
): Promise<readonly string[]> => {
  const result = await sandbox.exec({
    cwd: workspaceRoot,
    cmd: ['find', root, ...GUIDANCE_PRUNE_ARGS, '-type', type, '-print'],
  });

  if (result.exitCode !== 0) {
    return [];
  }

  return lines(result.stdout).map(normalizePath).sort();
};

const readIgnores = async (
  options: ExplorationOptions,
  root: string,
): Promise<readonly IgnorePattern[]> => {
  const result = await options.sandbox.exec({
    cwd: options.workspaceRoot,
    cmd: [
      'find',
      root,
      ...GUIDANCE_PRUNE_ARGS,
      '-type',
      'f',
      '-name',
      '.gitignore',
      '-print',
    ],
  });

  if (result.exitCode !== 0) {
    return [];
  }

  const files = lines(result.stdout)
    .map(normalizePath)
    .filter((file) => !isForbiddenAbsolutePath(options.workspaceRoot, file))
    .sort();
  const groups = await Promise.all(
    files.map(async (file) =>
      parseIgnores(
        path.dirname(file),
        await options.sandbox.readFile(file).catch(() => ''),
      ),
    ),
  );

  return groups.flat();
};

const parseIgnores = (base: string, text: string): readonly IgnorePattern[] =>
  text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line !== '' && !line.startsWith('#'))
    .map((line) => ({
      base,
      pattern: line.startsWith('!') ? line.slice(1) : line,
      negated: line.startsWith('!'),
    }))
    .filter((ignore) => ignore.pattern !== '');

const isIgnoredWithAncestors = (
  root: string,
  fullPath: string,
  isDirectory: boolean,
  ignores: readonly IgnorePattern[],
): boolean => {
  const ignoredAncestor = ancestors(root, fullPath).some((ancestor) =>
    isIgnored(ancestor, true, ignores),
  );

  return ignoredAncestor || isIgnored(fullPath, isDirectory, ignores);
};

const ancestors = (root: string, fullPath: string): readonly string[] => {
  const values: string[] = [];
  let current = path.dirname(fullPath);

  while (contains(root, current) && current !== root) {
    values.unshift(current);
    current = path.dirname(current);
  }

  return values;
};

const parsePathKind = (value: string): PathKind => {
  const normalized = value.trim();

  if (
    normalized === 'directory' ||
    normalized === 'file' ||
    normalized === 'missing' ||
    normalized === 'other'
  ) {
    return normalized;
  }

  return 'missing';
};

const isHidden = (root: string, fullPath: string): boolean =>
  path
    .relative(root, fullPath)
    .split('/')
    .some((part) => part.startsWith('.'));

const isExcluded = (fullPath: string, excludes: readonly RegExp[]): boolean => {
  const name = path.basename(fullPath);

  return excludes.some(
    (pattern) => pattern.test(name) || pattern.test(fullPath),
  );
};

const contains = (root: string, child: string): boolean =>
  child === root || child.startsWith(`${root}/`);

const lines = (value: string): readonly string[] =>
  value.split(/\r?\n/).filter((line) => line !== '');

const normalizePath = (value: string): string => {
  const resolved = path.normalize(path.isAbsolute(value) ? value : `/${value}`);

  return resolved === '/' ? resolved : resolved.replace(/\/+$/, '');
};
