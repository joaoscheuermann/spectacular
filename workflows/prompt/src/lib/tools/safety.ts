import { readFile } from 'node:fs/promises';
import path from 'node:path';

export type IgnorePattern = {
  readonly base: string;
  readonly pattern: string;
  readonly negated: boolean;
};

export type SafeTarget =
  | {
      readonly absolutePath: string;
      readonly relativePath: string;
      readonly displayPath: string;
    }
  | {
      readonly error: string;
    };

export const FORBIDDEN_DIRECT_MESSAGE =
  'Prompt workflow exploration cannot access repository guidance markdown such as GROUNDING.md, AGENTS.md, README files, docs/**, .agents/**, or other markdown files.';

export const TREE_SAFE_EXCLUDES = [
  'docs',
  '.agents',
  '*.md',
  '*.mdx',
  '*.markdown',
  'README*',
  'AGENTS.md',
  'GROUNDING.md',
] as const;

export const resolveTarget = (
  workspaceRoot: string,
  value: string,
): SafeTarget => {
  const root = path.resolve(workspaceRoot);
  const absolutePath = path.resolve(
    path.isAbsolute(value) ? value : path.join(root, value),
  );
  const relative = path.relative(root, absolutePath);

  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    return {
      error:
        'Prompt workflow exploration is limited to the configured workspace root.',
    };
  }

  const relativePath = toPosix(relative);

  return {
    absolutePath,
    relativePath,
    displayPath: relativePath === '' ? '.' : relativePath,
  };
};

export const isForbiddenAbsolutePath = (
  workspaceRoot: string,
  value: string,
): boolean => {
  const target = resolveTarget(workspaceRoot, value);

  return 'error' in target || isForbiddenRelativePath(target.relativePath);
};

export const isForbiddenRelativePath = (value: string): boolean => {
  const normalized = toPosix(value).replace(/^\.\//, '').toLowerCase();
  if (normalized === '' || normalized === '.') {
    return false;
  }

  const segments = normalized.split('/').filter(Boolean);

  return (
    segments.some((segment) => segment === 'docs' || segment === '.agents') ||
    isForbiddenName(segments.at(-1) ?? '')
  );
};

export const isForbiddenPattern = (pattern: string): boolean => {
  const normalized = pattern.replaceAll('\\', '/').toLowerCase();
  const segments = normalized.split('/').filter(Boolean);

  return (
    segments.some((segment) => segment === 'docs' || segment === '.agents') ||
    segments.some(isForbiddenName)
  );
};

export const isClearlyNonMarkdownGlob = (glob: string | undefined): boolean => {
  if (glob === undefined || isForbiddenPattern(glob)) {
    return false;
  }

  return /\.[a-z0-9]+$/i.test(glob);
};

export const readIgnores = async (
  dir: string,
): Promise<readonly IgnorePattern[]> => {
  const text = await readFile(path.join(dir, '.gitignore'), 'utf8').catch(
    () => '',
  );

  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line !== '' && !line.startsWith('#'))
    .map((line) => ({
      base: dir,
      pattern: line.startsWith('!') ? line.slice(1) : line,
      negated: line.startsWith('!'),
    }))
    .filter((ignore) => ignore.pattern !== '');
};

export const isIgnored = (
  fullPath: string,
  isDirectory: boolean,
  ignores: readonly IgnorePattern[],
): boolean => {
  let ignored = false;
  for (const ignore of ignores) {
    const pattern = ignore.pattern.endsWith('/')
      ? ignore.pattern.slice(0, -1)
      : ignore.pattern;
    if (ignore.pattern.endsWith('/') && !isDirectory) {
      continue;
    }

    const glob = compileGlob(pattern);
    const relative = toPosix(path.relative(ignore.base, fullPath));
    const matched =
      glob instanceof RegExp
        ? glob.test(relative) || glob.test(path.basename(fullPath))
        : relative === pattern || path.basename(fullPath) === pattern;
    if (matched) {
      ignored = !ignore.negated;
    }
  }

  return ignored;
};

export const compileGlob = (pattern: string): RegExp | string => {
  if (pattern.includes('[') || pattern.includes(']')) {
    return pattern.includes('[') && !pattern.includes(']')
      ? 'unclosed character class'
      : 'unsupported character classes';
  }

  const source = pattern
    .split('')
    .map((char, index, chars) => {
      if (char === '*' && chars[index + 1] === '*') {
        return '\0';
      }
      if (char === '*' && chars[index - 1] === '*') {
        return '';
      }
      if (char === '*') {
        return '[^/]*';
      }
      if (char === '?') {
        return '[^/]';
      }
      if (char === '\0') {
        return '.*';
      }

      return escapeRegExp(char);
    })
    .join('')
    .replaceAll('\0', '.*');

  return new RegExp(`^${source}$`);
};

export const escapeRegExp = (value: string): string =>
  value.replace(/[\\^$+?.()|{}]/g, '\\$&');

export const toPosix = (value: string): string =>
  value.replaceAll(path.sep, '/');

const isForbiddenName = (name: string): boolean =>
  name === 'agents.md' ||
  name === 'grounding.md' ||
  name === 'readme' ||
  name.startsWith('readme.') ||
  name.endsWith('.md') ||
  name.endsWith('.mdx') ||
  name.endsWith('.markdown');
