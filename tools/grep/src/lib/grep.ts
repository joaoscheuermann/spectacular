import { readdir, readFile, stat } from 'node:fs/promises';
import path from 'node:path';

import { createTool as defineTool } from 'tools';
import { z } from 'zod';

const DEFAULT_LIMIT = 100;
const MAX_OUTPUT_BYTES = 50 * 1024;
const MAX_LINE_LENGTH = 500;

const description =
  'Search file contents for a pattern. Returns matching lines with file paths and line numbers. Respects .gitignore. Output is truncated to 100 matches or 50KB (whichever is hit first). Long lines are truncated to 500 chars.';

export const schema = z
  .object({
    pattern: z.string(),
    path: z.string().optional(),
    glob: z.string().optional(),
    ignoreCase: z.boolean().optional(),
    literal: z.boolean().optional(),
    context: z.number().int().nonnegative().optional(),
    limit: z.number().int().nonnegative().optional(),
  })
  .strict();

export type GrepMatch = {
  readonly file: string;
  readonly line: number;
  readonly text: string;
  readonly context_before: readonly string[];
  readonly context_after: readonly string[];
};

export type GrepOutput = {
  readonly matches: readonly GrepMatch[];
  readonly total: number;
  readonly truncated: boolean;
  readonly lines_truncated: boolean;
  readonly error?: string;
};

type Options = {
  readonly workspaceRoot: string;
};

type IgnorePattern = {
  readonly base: string;
  readonly pattern: string;
  readonly negated: boolean;
};

/** Creates the provider-neutral content search tool. */
export const createTool = ({ workspaceRoot }: Options) =>
  defineTool({
    name: 'grep',
    description,
    schema,
    execute: (input): Promise<GrepOutput> => execute(workspaceRoot, input),
  });

const execute = async (
  workspaceRoot: string,
  input: z.output<typeof schema>,
): Promise<GrepOutput> => {
  const searchDir = input.path ?? '.';
  const searchPath = resolvePath(workspaceRoot, searchDir);

  if (!(await exists(searchPath))) {
    return empty(`Path not found: ${searchDir}`);
  }

  const regex = compileSearch(input);
  if (typeof regex === 'string') {
    return empty(`Invalid regex pattern: ${regex}`);
  }

  const glob = input.glob === undefined ? undefined : compileGlob(input.glob);
  if (typeof glob === 'string') {
    return empty(`Invalid glob pattern '${input.glob}': ${glob}`);
  }

  const files = (await stat(searchPath)).isFile()
    ? [searchPath]
    : await collectFiles(searchPath, []);

  return collect(
    searchPath,
    files,
    regex,
    glob,
    input.context ?? 0,
    Math.max(input.limit ?? DEFAULT_LIMIT, 1),
  );
};

const collect = async (
  searchPath: string,
  files: readonly string[],
  regex: RegExp,
  glob: RegExp | undefined,
  context: number,
  limit: number,
): Promise<GrepOutput> => {
  const isSingleFile = (await stat(searchPath)).isFile();
  const matches: GrepMatch[] = [];
  let totalBytes = 0;
  let truncated = false;
  let linesTruncated = false;

  for (const file of files) {
    const relative = isSingleFile
      ? path.basename(file)
      : toPosix(path.relative(searchPath, file));
    if (
      glob !== undefined &&
      !glob.test(relative) &&
      !glob.test(path.basename(file))
    ) {
      continue;
    }

    const text = await readFile(file, 'utf8').catch(() => undefined);
    if (text === undefined) {
      continue;
    }

    const lines = text.split(/\r?\n/);
    for (const [index, line] of lines.entries()) {
      regex.lastIndex = 0;
      if (!regex.test(line)) {
        continue;
      }

      const [matched, matchTruncated] = truncateLine(line);
      const [before, beforeTruncated] = contextBefore(lines, index, context);
      const [after, afterTruncated] = contextAfter(lines, index, context);
      linesTruncated ||= matchTruncated || beforeTruncated || afterTruncated;

      const entryBytes = relative.length + matched.length + 20;
      if (
        totalBytes + entryBytes > MAX_OUTPUT_BYTES ||
        matches.length >= limit
      ) {
        truncated = true;
        break;
      }

      totalBytes += entryBytes;
      matches.push({
        file: relative,
        line: index + 1,
        text: matched,
        context_before: before,
        context_after: after,
      });
    }

    if (truncated) {
      break;
    }
  }

  return {
    matches,
    total: matches.length,
    truncated,
    lines_truncated: linesTruncated,
  };
};

const collectFiles = async (
  dir: string,
  parentIgnores: readonly IgnorePattern[],
): Promise<readonly string[]> => {
  const ignores = [...parentIgnores, ...(await readIgnores(dir))];
  const entries = await readdir(dir, { withFileTypes: true }).catch(() => []);
  const nested = await Promise.all(
    entries.map(async (entry) => {
      if (entry.name === '.git' || entry.name === 'node_modules') {
        return [];
      }

      const fullPath = path.join(dir, entry.name);
      if (isIgnored(fullPath, entry.isDirectory(), ignores)) {
        return [];
      }

      if (entry.isDirectory()) {
        return collectFiles(fullPath, ignores);
      }

      return entry.isFile() ? [fullPath] : [];
    }),
  );

  return nested.flat();
};

const compileSearch = (input: z.output<typeof schema>): RegExp | string => {
  const pattern =
    input.literal === true ? escapeRegExp(input.pattern) : input.pattern;
  try {
    return new RegExp(pattern, input.ignoreCase === true ? 'i' : undefined);
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
};

const contextBefore = (
  lines: readonly string[],
  index: number,
  count: number,
): [readonly string[], boolean] =>
  contextLines(lines.slice(Math.max(index - count, 0), index));

const contextAfter = (
  lines: readonly string[],
  index: number,
  count: number,
): [readonly string[], boolean] =>
  contextLines(lines.slice(index + 1, index + 1 + count));

const contextLines = (
  lines: readonly string[],
): [readonly string[], boolean] => {
  let truncated = false;
  const values = lines.map((line) => {
    const [value, wasTruncated] = truncateLine(line);
    truncated ||= wasTruncated;
    return value;
  });

  return [values, truncated];
};

const truncateLine = (line: string): [string, boolean] => {
  if (line.length <= MAX_LINE_LENGTH) {
    return [line, false];
  }

  return [`${line.slice(0, MAX_LINE_LENGTH)}... [truncated]`, true];
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

const compileGlob = (pattern: string): RegExp | string => {
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

const resolvePath = (workspaceRoot: string, value: string): string =>
  path.normalize(
    path.isAbsolute(value) ? value : path.join(workspaceRoot, value),
  );

const exists = async (value: string): Promise<boolean> =>
  stat(value).then(
    () => true,
    () => false,
  );

const empty = (error: string): GrepOutput => ({
  matches: [],
  total: 0,
  truncated: false,
  lines_truncated: false,
  error,
});

const escapeRegExp = (value: string): string =>
  value.replace(/[\\^$+?.()|{}]/g, '\\$&');
const toPosix = (value: string): string => value.replaceAll(path.sep, '/');
