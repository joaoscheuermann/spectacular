import { lstat, readdir, readFile } from 'node:fs/promises';
import path from 'node:path';

import { createTool as defineTool } from 'tools';
import { z } from 'zod';

const HIDDEN_EXCEPTIONS = new Set(['.agents']);

const description =
  'Display directory structure as an ASCII tree. Directories are listed first, then files, both sorted alphabetically. Respects .gitignore and excludes hidden files except .agents.';

export const schema = z
  .object({
    path: z.string().optional(),
    exclude: z.array(z.string()).optional(),
  })
  .strict();

type Options = {
  readonly workspaceRoot: string;
};

type Entry = {
  readonly path: string;
  readonly isDirectory: boolean;
};

type IgnorePattern = {
  readonly base: string;
  readonly pattern: string;
  readonly negated: boolean;
};

/** Creates the provider-neutral ASCII tree tool. */
export const createTool = ({ workspaceRoot }: Options) =>
  defineTool({
    name: 'tree',
    description,
    schema,
    execute: (input): Promise<string> => execute(workspaceRoot, input),
  });

const execute = async (
  workspaceRoot: string,
  input: z.output<typeof schema>,
): Promise<string> => {
  const rawPath = input.path ?? '';
  const displayPath = rawPath === '' ? '.' : rawPath;
  const root = resolvePath(workspaceRoot, displayPath);
  const rootStat = await lstat(root).catch(() => undefined);

  if (rootStat === undefined) {
    return `Error: path not found: ${displayPath}`;
  }

  if (!rootStat.isDirectory()) {
    return `Error: path is not a directory: ${displayPath}`;
  }

  const excludes = input.exclude?.map(compileGlob) ?? [];
  const invalid = excludes.find(
    (value): value is string => typeof value === 'string',
  );
  if (invalid !== undefined) {
    return invalid;
  }

  const rootName = path.basename(root);
  if ((excludes as RegExp[]).some((pattern) => pattern.test(rootName))) {
    return `Error: exclude pattern matches the root directory: ${rootName}`;
  }

  const lines = [toPosix(path.resolve(root))];
  await appendTree(root, '', [], excludes as RegExp[], lines);
  return `${lines.join('\n')}\n`;
};

const appendTree = async (
  dir: string,
  prefix: string,
  parentIgnores: readonly IgnorePattern[],
  excludes: readonly RegExp[],
  lines: string[],
): Promise<void> => {
  const ignores = [...parentIgnores, ...(await readIgnores(dir))];
  const entries = await sortedEntries(dir, ignores, excludes);

  for (const [index, entry] of entries.entries()) {
    const isLast = index === entries.length - 1;
    const connector = isLast ? '`-- ' : '|-- ';
    const name = path.basename(entry.path);

    if (!entry.isDirectory) {
      lines.push(`${prefix}${connector}${name}`);
      continue;
    }

    const mark = lines.length;
    lines.push(`${prefix}${connector}${name}/`);
    const childPrefix = isLast ? `${prefix}    ` : `${prefix}|   `;
    await appendTree(entry.path, childPrefix, ignores, excludes, lines);

    if (lines.length === mark + 1) {
      lines[mark] = `${lines[mark]} (empty)`;
    }
  }
};

const sortedEntries = async (
  dir: string,
  ignores: readonly IgnorePattern[],
  excludes: readonly RegExp[],
): Promise<readonly Entry[]> => {
  const entries = await readdir(dir, { withFileTypes: true }).catch(() => []);
  const visible = await Promise.all(
    entries.map(async (entry): Promise<Entry | undefined> => {
      const name = entry.name;
      if (name.startsWith('.') && !HIDDEN_EXCEPTIONS.has(name)) {
        return undefined;
      }

      const fullPath = path.join(dir, name);
      const stats = await lstat(fullPath).catch(() => undefined);
      if (stats === undefined || stats.isSymbolicLink()) {
        return undefined;
      }

      const isDirectory = stats.isDirectory();
      if (
        isIgnored(fullPath, isDirectory, ignores) ||
        isExcluded(fullPath, excludes)
      ) {
        return undefined;
      }

      return stats.isFile() || isDirectory
        ? { path: fullPath, isDirectory }
        : undefined;
    }),
  );

  return visible
    .filter((entry): entry is Entry => entry !== undefined)
    .sort((left, right) => {
      if (left.isDirectory !== right.isDirectory) {
        return left.isDirectory ? -1 : 1;
      }

      return path
        .basename(left.path)
        .localeCompare(path.basename(right.path), undefined, {
          sensitivity: 'accent',
        });
    });
};

const readIgnores = async (dir: string): Promise<readonly IgnorePattern[]> => {
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

const isIgnored = (
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

const isExcluded = (fullPath: string, excludes: readonly RegExp[]): boolean => {
  const name = path.basename(fullPath);
  const posixPath = toPosix(fullPath);
  return excludes.some(
    (pattern) => pattern.test(name) || pattern.test(posixPath),
  );
};

const compileGlob = (pattern: string): RegExp | string => {
  if (pattern.includes('[') || pattern.includes(']')) {
    const reason =
      pattern.includes('[') && !pattern.includes(']')
        ? 'unclosed character class'
        : 'unsupported character classes';
    return `Error: invalid exclude pattern '${pattern}': ${reason}`;
  }

  const source = pattern
    .replace(/[\\^$+?.()|{}]/g, '\\$&')
    .replaceAll('**', '\0')
    .replaceAll('*', '[^/]*')
    .replaceAll('?', '[^/]')
    .replaceAll('\0', '.*');

  return new RegExp(`^${source}$`);
};

const resolvePath = (workspaceRoot: string, value: string): string =>
  path.normalize(
    path.isAbsolute(value) ? value : path.join(workspaceRoot, value),
  );

const toPosix = (value: string): string => value.replaceAll(path.sep, '/');
