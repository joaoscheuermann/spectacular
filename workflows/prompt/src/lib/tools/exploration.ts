import { lstat, readdir, readFile, stat } from 'node:fs/promises';
import path from 'node:path';

import { createTool as createFindTool, schema as findSchema } from 'tool-find';
import { schema as grepSchema } from 'tool-grep';
import { createTool as createTreeTool, schema as treeSchema } from 'tool-tree';
import { createTool, createToolStorage } from 'tools';
import type { ToolStorage } from 'tools';
import type { z } from 'zod';

import {
  FORBIDDEN_DIRECT_MESSAGE,
  TREE_SAFE_EXCLUDES,
  compileGlob,
  escapeRegExp,
  isClearlyNonMarkdownGlob,
  isForbiddenAbsolutePath,
  isForbiddenPattern,
  isForbiddenRelativePath,
  isIgnored,
  readIgnores,
  resolveTarget,
  toPosix,
  type IgnorePattern,
} from './safety.js';

type FindInput = z.output<typeof findSchema>;
type GrepInput = z.output<typeof grepSchema>;
type TreeInput = z.output<typeof treeSchema>;

type FindOutput = {
  readonly results: readonly string[];
  readonly total: number;
  readonly truncated: boolean;
  readonly error?: string;
};

type GrepMatch = {
  readonly file: string;
  readonly line: number;
  readonly text: string;
  readonly context_before: readonly string[];
  readonly context_after: readonly string[];
};

type GrepOutput = {
  readonly matches: readonly GrepMatch[];
  readonly total: number;
  readonly truncated: boolean;
  readonly lines_truncated: boolean;
  readonly error?: string;
};

const DEFAULT_FIND_LIMIT = 1000;
const DEFAULT_GREP_LIMIT = 100;
const MAX_OUTPUT_BYTES = 50 * 1024;
const MAX_LINE_LENGTH = 500;

const SAFE_GREP_USAGE =
  'Prompt workflow grep requires a clearly non-markdown file path, or a non-markdown glob such as **/*.ts, **/*.json, or **/*.js. Broad repository searches are blocked so guidance markdown is never read.';

/** Creates read-only exploration tools that cannot load repository guidance markdown. */
export const createSafeExplorationTools = (
  workspaceRoot: string,
): ToolStorage =>
  createToolStorage([
    createSafeFindTool(workspaceRoot),
    createSafeGrepTool(workspaceRoot),
    createSafeTreeTool(workspaceRoot),
  ]);

const createSafeFindTool = (workspaceRoot: string) => {
  const base = createFindTool({ workspaceRoot });

  return createTool({
    name: base.name,
    description:
      'Search non-guidance files by glob pattern. Guidance markdown, docs, README files, AGENTS.md, GROUNDING.md, and .agents are excluded.',
    schema: findSchema,
    execute: (input): Promise<FindOutput> => find(workspaceRoot, input),
  });
};

const createSafeGrepTool = (workspaceRoot: string) =>
  createTool({
    name: 'grep',
    description:
      'Search non-guidance file contents. Requires a non-markdown file path or clearly non-markdown glob.',
    schema: grepSchema,
    execute: (input): Promise<GrepOutput> => grep(workspaceRoot, input),
  });

const createSafeTreeTool = (workspaceRoot: string) => {
  const base = createTreeTool({ workspaceRoot });

  return createTool({
    name: base.name,
    description:
      'Display directory structure without repository guidance markdown, docs, README files, AGENTS.md, GROUNDING.md, or .agents.',
    schema: treeSchema,
    execute: async (input): Promise<string> => {
      const target = resolveTarget(workspaceRoot, input.path ?? '.');
      if ('error' in target) {
        return `Error: ${target.error}`;
      }

      if (isForbiddenRelativePath(target.relativePath)) {
        return `Error: ${FORBIDDEN_DIRECT_MESSAGE}`;
      }

      const tree = await base.execute({
        ...input,
        exclude: [...TREE_SAFE_EXCLUDES, ...(input.exclude ?? [])],
      } satisfies TreeInput);

      return filterTree(tree);
    },
  });
};

const find = async (
  workspaceRoot: string,
  input: FindInput,
): Promise<FindOutput> => {
  const target = resolveTarget(workspaceRoot, input.path ?? '.');
  if ('error' in target) {
    return emptyFind(target.error);
  }

  if (
    isForbiddenRelativePath(target.relativePath) ||
    isForbiddenPattern(input.pattern)
  ) {
    return emptyFind(FORBIDDEN_DIRECT_MESSAGE);
  }

  const rootStat = await lstat(target.absolutePath).catch(() => undefined);
  if (rootStat === undefined) {
    return emptyFind(`Path not found: ${target.displayPath}`);
  }

  if (!rootStat.isDirectory()) {
    return emptyFind(`Path is not a directory: ${target.displayPath}`);
  }

  const glob = compileGlob(input.pattern);
  if (typeof glob === 'string') {
    return emptyFind(`Invalid glob pattern '${input.pattern}': ${glob}`);
  }

  return collectFind(
    workspaceRoot,
    target.absolutePath,
    glob,
    input.limit ?? DEFAULT_FIND_LIMIT,
  );
};

const grep = async (
  workspaceRoot: string,
  input: GrepInput,
): Promise<GrepOutput> => {
  const target = resolveTarget(workspaceRoot, input.path ?? '.');
  if ('error' in target) {
    return emptyGrep(target.error);
  }

  if (isForbiddenRelativePath(target.relativePath)) {
    return emptyGrep(FORBIDDEN_DIRECT_MESSAGE);
  }

  const rootStat = await stat(target.absolutePath).catch(() => undefined);
  if (rootStat === undefined) {
    return emptyGrep(`Path not found: ${target.displayPath}`);
  }

  if (!rootStat.isFile() && !isClearlyNonMarkdownGlob(input.glob)) {
    return emptyGrep(SAFE_GREP_USAGE);
  }

  const regex = compileSearch(input);
  if (typeof regex === 'string') {
    return emptyGrep(`Invalid regex pattern: ${regex}`);
  }

  const glob = input.glob === undefined ? undefined : compileGlob(input.glob);
  if (typeof glob === 'string') {
    return emptyGrep(`Invalid glob pattern '${input.glob}': ${glob}`);
  }

  const files = rootStat.isFile()
    ? [target.absolutePath]
    : await collectFiles(workspaceRoot, target.absolutePath, []);

  return collectGrep(
    target.absolutePath,
    files,
    regex,
    glob,
    input.context ?? 0,
    Math.max(input.limit ?? DEFAULT_GREP_LIMIT, 1),
  );
};

const collectFind = async (
  workspaceRoot: string,
  searchPath: string,
  glob: RegExp,
  limit: number,
): Promise<FindOutput> => {
  const results: string[] = [];
  let totalBytes = 0;
  let totalMatched = 0;
  let truncated = false;

  for await (const file of walkFiles(workspaceRoot, searchPath, [])) {
    const relative = toPosix(path.relative(searchPath, file));
    const name = path.basename(file);

    if (!glob.test(relative) && !glob.test(name)) {
      continue;
    }

    totalMatched += 1;
    const lineBytes = relative.length + 1;
    if (totalBytes + lineBytes > MAX_OUTPUT_BYTES || results.length >= limit) {
      truncated = true;
      break;
    }

    totalBytes += lineBytes;
    results.push(relative);
  }

  return {
    results,
    total: truncated ? totalMatched : results.length,
    truncated,
  };
};

const collectGrep = async (
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
  workspaceRoot: string,
  dir: string,
  parentIgnores: readonly IgnorePattern[],
): Promise<readonly string[]> => {
  const ignores = [...parentIgnores, ...(await readIgnores(dir))];
  const entries = await readdir(dir, { withFileTypes: true }).catch(() => []);
  const nested = await Promise.all(
    entries.map(async (entry) => {
      const fullPath = path.join(dir, entry.name);

      if (
        entry.name === '.git' ||
        entry.name === 'node_modules' ||
        isForbiddenAbsolutePath(workspaceRoot, fullPath) ||
        isIgnored(fullPath, entry.isDirectory(), ignores)
      ) {
        return [];
      }

      if (entry.isDirectory()) {
        return collectFiles(workspaceRoot, fullPath, ignores);
      }

      return entry.isFile() ? [fullPath] : [];
    }),
  );

  return nested.flat();
};

async function* walkFiles(
  workspaceRoot: string,
  dir: string,
  parentIgnores: readonly IgnorePattern[],
): AsyncGenerator<string> {
  const ignores = [...parentIgnores, ...(await readIgnores(dir))];
  const entries = await readdir(dir, { withFileTypes: true }).catch(() => []);

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);

    if (
      entry.name === '.git' ||
      entry.name === 'node_modules' ||
      isForbiddenAbsolutePath(workspaceRoot, fullPath) ||
      isIgnored(fullPath, entry.isDirectory(), ignores)
    ) {
      continue;
    }

    if (entry.isDirectory()) {
      yield* walkFiles(workspaceRoot, fullPath, ignores);
    } else if (entry.isFile()) {
      yield fullPath;
    }
  }
}

const compileSearch = (input: GrepInput): RegExp | string => {
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

const filterTree = (tree: string): string => {
  const lines = tree
    .split(/\r?\n/)
    .filter((line) => !isForbiddenTreeLine(line));

  return lines.join('\n');
};

const isForbiddenTreeLine = (line: string): boolean => {
  const name = line
    .trim()
    .replace(/^[|`\-\s]+/, '')
    .replace(/ \(empty\)$/, '')
    .replace(/\/$/, '')
    .toLowerCase();

  return isForbiddenRelativePath(name);
};

const emptyFind = (error: string): FindOutput => ({
  results: [],
  total: 0,
  truncated: false,
  error,
});

const emptyGrep = (error: string): GrepOutput => ({
  matches: [],
  total: 0,
  truncated: false,
  lines_truncated: false,
  error,
});
