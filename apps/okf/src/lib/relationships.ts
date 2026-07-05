import path from 'node:path';

import {
  normalizeRelativePath,
  parentRelativePath,
  posixJoin,
} from './paths.js';
import type { AnalyzedFile } from './types.js';

const resolvableExtensions = [
  '.ts',
  '.tsx',
  '.js',
  '.jsx',
  '.mjs',
  '.cjs',
  '.json',
  '.md',
  '.py',
  '.rs',
];

export const relationshipCandidatePaths = (
  file: AnalyzedFile,
  specifier: string,
): readonly string[] => {
  if (file.extension === '.py') {
    return pythonCandidatePaths(file.relativePath, specifier);
  }

  if (file.extension === '.rs') {
    return rustCandidatePaths(file.relativePath, specifier);
  }

  const base = parentRelativePath(file.relativePath);

  return jsCandidatePaths(normalizeRelativePath(posixJoin(base, specifier)));
};

const jsCandidatePaths = (target: string): readonly string[] => {
  const extension = path.posix.extname(target);
  const extensionless =
    extension === '' ? target : target.slice(0, -extension.length);

  return uniquePaths([
    target,
    ...resolvableExtensions.map((item) => `${target}${item}`),
    ...resolvableExtensions.map((item) => `${extensionless}${item}`),
    ...resolvableExtensions.map((item) => posixJoin(target, `index${item}`)),
  ]);
};

const pythonCandidatePaths = (
  fromRelativePath: string,
  specifier: string,
): readonly string[] => {
  const modulePath = specifier.replace(/^\.+/u, '').replaceAll('.', '/');

  if (modulePath === '') {
    return [];
  }

  if (specifier.startsWith('.')) {
    return pythonModuleCandidates(
      posixJoin(pythonRelativeBase(fromRelativePath, specifier), modulePath),
    );
  }

  return uniquePaths([
    ...pythonModuleCandidates(
      posixJoin(parentRelativePath(fromRelativePath), modulePath),
    ),
    ...pythonModuleCandidates(modulePath),
  ]);
};

const pythonRelativeBase = (
  fromRelativePath: string,
  specifier: string,
): string => {
  const leadingDots = specifier.match(/^\.+/u)?.[0].length ?? 0;
  let base = parentRelativePath(fromRelativePath);

  for (let index = 1; index < leadingDots; index += 1) {
    base = parentRelativePath(base);
  }

  return base;
};

const pythonModuleCandidates = (target: string): readonly string[] =>
  path.posix.extname(target) === '.py'
    ? [target]
    : [target, `${target}.py`, posixJoin(target, '__init__.py')];

const rustCandidatePaths = (
  fromRelativePath: string,
  specifier: string,
): readonly string[] => {
  const normalized = specifier.split(/\s+as\s+/u)[0]?.trim() ?? '';

  if (normalized === '') {
    return [];
  }

  if (normalized.startsWith('crate::')) {
    return rustModuleCandidates(
      rustCrateRoot(fromRelativePath),
      rustSegments(normalized.slice('crate::'.length)),
    );
  }

  if (normalized.startsWith('self::')) {
    return rustModuleCandidates(
      rustSelfBase(fromRelativePath),
      rustSegments(normalized.slice('self::'.length)),
    );
  }

  if (normalized.startsWith('super::')) {
    return rustModuleCandidates(
      rustSuperBase(fromRelativePath),
      rustSegments(normalized.slice('super::'.length)),
    );
  }

  const segments = rustSegments(normalized);

  return uniquePaths([
    ...rustModuleCandidates(parentRelativePath(fromRelativePath), segments),
    ...rustModuleCandidates(rustCrateRoot(fromRelativePath), segments),
  ]);
};

const rustSegments = (specifier: string): readonly string[] =>
  specifier
    .split('::')
    .map((segment) => segment.trim())
    .filter(Boolean);

const rustModuleCandidates = (
  basePath: string,
  segments: readonly string[],
): readonly string[] =>
  uniquePaths(
    segments
      .flatMap((_, index) =>
        rustFileCandidates(
          posixJoin(basePath, ...segments.slice(0, index + 1)),
        ),
      )
      .reverse(),
  );

const rustFileCandidates = (target: string): readonly string[] =>
  path.posix.extname(target) === '.rs'
    ? [target]
    : [target, `${target}.rs`, posixJoin(target, 'mod.rs')];

const rustCrateRoot = (fromRelativePath: string): string => {
  const fileSegments = normalizeRelativePath(fromRelativePath)
    .split('/')
    .slice(0, -1);
  const srcIndex = fileSegments.lastIndexOf('src');

  return srcIndex === -1 ? '' : fileSegments.slice(0, srcIndex + 1).join('/');
};

const rustSelfBase = (fromRelativePath: string): string => {
  const basename = path.posix.basename(fromRelativePath, '.rs');
  const folder = parentRelativePath(fromRelativePath);

  return basename === 'mod' || basename === 'lib' || basename === 'main'
    ? folder
    : posixJoin(folder, basename);
};

const rustSuperBase = (fromRelativePath: string): string => {
  const basename = path.posix.basename(fromRelativePath);
  const folder = parentRelativePath(fromRelativePath);

  return basename === 'mod.rs' ? parentRelativePath(folder) : folder;
};

const uniquePaths = (values: readonly string[]): readonly string[] => [
  ...new Set(values.filter((value) => value !== '')),
];
