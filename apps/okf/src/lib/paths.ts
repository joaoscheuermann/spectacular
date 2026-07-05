import path from 'node:path';

export const KNOWLEDGE_DIR = '.doric/knowledge';

export const normalizeRelativePath = (value: string): string =>
  value
    .replaceAll('\\', '/')
    .replace(/^\.\/+/, '')
    .replace(/^\/+/, '')
    .replace(/\/+$/, '');

export const toPosixRelative = (
  rootPath: string,
  absolutePath: string,
): string => normalizeRelativePath(path.relative(rootPath, absolutePath));

export const posixJoin = (...parts: readonly string[]): string =>
  normalizeRelativePath(
    path.posix.join(...parts.filter((part) => part !== '')),
  );

export const parentRelativePath = (relativePath: string): string => {
  const parent = path.posix.dirname(normalizeRelativePath(relativePath));

  return parent === '.' ? '' : parent;
};

export const outputRelativePath = (
  folderRelativePath: string,
  hash: string,
): string => posixJoin(folderRelativePath, `${hash}.md`);

export const outputAbsolutePath = (
  rootPath: string,
  outputPath: string,
): string => path.join(rootPath, KNOWLEDGE_DIR, ...outputPath.split('/'));

export const outputFolderPath = (
  rootPath: string,
  folderRelativePath: string,
): string =>
  path.join(
    rootPath,
    KNOWLEDGE_DIR,
    ...normalizeRelativePath(folderRelativePath).split('/').filter(Boolean),
  );

export const isAgentsPath = (relativePath: string): boolean => {
  const normalized = normalizeRelativePath(relativePath);

  return normalized === '.agents' || normalized.startsWith('.agents/');
};

export const isDoricPath = (relativePath: string): boolean => {
  const normalized = normalizeRelativePath(relativePath);

  return normalized === '.doric' || normalized.startsWith('.doric/');
};

export const isGitPath = (relativePath: string): boolean => {
  const normalized = normalizeRelativePath(relativePath);

  return normalized === '.git' || normalized.startsWith('.git/');
};
