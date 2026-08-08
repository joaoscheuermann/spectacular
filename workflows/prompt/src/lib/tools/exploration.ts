import { posix as path } from 'node:path';

import { createToolStorage, defineTool } from 'tool';
import type { ToolStorage } from 'tool';
import type { z } from 'zod';
import { z as zod } from 'zod';

import { collectFind, type FindOutput } from './find-results.js';
import { collectGrep, compileSearch, type GrepOutput } from './grep-results.js';
import {
  pathKind,
  type ExplorationOptions as Options,
  visibleEntries,
  visibleFiles,
} from './sandbox-exploration.js';
import {
  FORBIDDEN_DIRECT_MESSAGE,
  compileGlob,
  isClearlyNonMarkdownGlob,
  isForbiddenPattern,
  isForbiddenRelativePath,
  resolveTarget,
} from './safety.js';
import { compileTreeExcludes, renderTree } from './tree-render.js';

type FindInput = z.output<typeof findSchema>;
type GrepInput = z.output<typeof grepSchema>;
type TreeInput = z.output<typeof treeSchema>;

const findSchema = zod
  .object({
    pattern: zod.string(),
    path: zod.string().optional(),
    limit: zod.number().int().nonnegative().optional(),
  })
  .strict();
const grepSchema = zod
  .object({
    pattern: zod.string(),
    path: zod.string().optional(),
    glob: zod.string().optional(),
    ignoreCase: zod.boolean().optional(),
    literal: zod.boolean().optional(),
    context: zod.number().int().nonnegative().optional(),
    limit: zod.number().int().nonnegative().optional(),
  })
  .strict();
const treeSchema = zod
  .object({
    path: zod.string().optional(),
    exclude: zod.array(zod.string()).optional(),
  })
  .strict();
const findOutput = zod
  .object({
    results: zod.array(zod.string()),
    total: zod.number(),
    truncated: zod.boolean(),
    error: zod.string().optional(),
  })
  .strict();
const grepOutput = zod
  .object({
    matches: zod.array(
      zod
        .object({
          file: zod.string(),
          line: zod.number(),
          text: zod.string(),
          context_before: zod.array(zod.string()),
          context_after: zod.array(zod.string()),
        })
        .strict(),
    ),
    total: zod.number(),
    truncated: zod.boolean(),
    lines_truncated: zod.boolean(),
    error: zod.string().optional(),
  })
  .strict();
const treeOutput = zod.string();

const DEFAULT_FIND_LIMIT = 1000;
const DEFAULT_GREP_LIMIT = 100;

const SAFE_GREP_USAGE =
  'Prompt workflow grep requires a clearly non-markdown file path, or a non-markdown glob such as **/*.ts, **/*.json, or **/*.js. Broad repository searches are blocked so guidance markdown is never read.';

/** Creates read-only exploration tools that cannot load repository guidance markdown. */
export const createSafeExplorationTools = (options: Options): ToolStorage =>
  createToolStorage([
    createSafeFindTool(options),
    createSafeGrepTool(options),
    createSafeTreeTool(options),
  ]);

const createSafeFindTool = (options: Options) => {
  const factory = defineTool({
    name: 'find',
    description:
      'Search non-guidance files by glob pattern. Guidance markdown, docs, README files, AGENTS.md, GROUNDING.md, and .agents are excluded.',
    input: findSchema,
    output: findOutput,
    execute: async (sandbox, input) => {
      const result = await find({ ...options, sandbox }, input);
      return { ...result, results: [...result.results] };
    },
  });

  return factory(options.sandbox);
};

const createSafeGrepTool = (options: Options) => {
  const factory = defineTool({
    name: 'grep',
    description:
      'Search non-guidance file contents. Requires a non-markdown file path or clearly non-markdown glob.',
    input: grepSchema,
    output: grepOutput,
    execute: async (sandbox, input) => {
      const result = await grep({ ...options, sandbox }, input);
      return {
        ...result,
        matches: result.matches.map((match) => ({
          ...match,
          context_before: [...match.context_before],
          context_after: [...match.context_after],
        })),
      };
    },
  });

  return factory(options.sandbox);
};

const createSafeTreeTool = (options: Options) => {
  const factory = defineTool({
    name: 'tree',
    description:
      'Display directory structure without repository guidance markdown, docs, README files, AGENTS.md, GROUNDING.md, or .agents.',
    input: treeSchema,
    output: treeOutput,
    execute: (sandbox, input): Promise<string> =>
      tree({ ...options, sandbox }, input),
  });

  return factory(options.sandbox);
};

const find = async (
  options: Options,
  input: FindInput,
): Promise<FindOutput> => {
  const target = resolveTarget(options.workspaceRoot, input.path ?? '.');
  if ('error' in target) {
    return emptyFind(target.error);
  }

  if (
    isForbiddenRelativePath(target.relativePath) ||
    isForbiddenPattern(input.pattern)
  ) {
    return emptyFind(FORBIDDEN_DIRECT_MESSAGE);
  }

  const kind = await pathKind(options, target.absolutePath);
  if (kind === 'missing') {
    return emptyFind(`Path not found: ${target.displayPath}`);
  }

  if (kind !== 'directory') {
    return emptyFind(`Path is not a directory: ${target.displayPath}`);
  }

  const glob = compileGlob(input.pattern);
  if (typeof glob === 'string') {
    return emptyFind(`Invalid glob pattern '${input.pattern}': ${glob}`);
  }

  return collectFind(
    target.absolutePath,
    await visibleFiles(options, target.absolutePath),
    glob,
    input.limit ?? DEFAULT_FIND_LIMIT,
  );
};

const grep = async (
  options: Options,
  input: GrepInput,
): Promise<GrepOutput> => {
  const target = resolveTarget(options.workspaceRoot, input.path ?? '.');
  if ('error' in target) {
    return emptyGrep(target.error);
  }

  if (isForbiddenRelativePath(target.relativePath)) {
    return emptyGrep(FORBIDDEN_DIRECT_MESSAGE);
  }

  const kind = await pathKind(options, target.absolutePath);
  if (kind === 'missing') {
    return emptyGrep(`Path not found: ${target.displayPath}`);
  }

  if (kind !== 'file' && !isClearlyNonMarkdownGlob(input.glob)) {
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

  const files =
    kind === 'file'
      ? [target.absolutePath]
      : await visibleFiles(options, target.absolutePath);

  return collectGrep(
    options.sandbox,
    target,
    files,
    kind === 'file',
    regex,
    glob,
    input.context ?? 0,
    Math.max(input.limit ?? DEFAULT_GREP_LIMIT, 1),
  );
};

const tree = async (options: Options, input: TreeInput): Promise<string> => {
  const target = resolveTarget(options.workspaceRoot, input.path ?? '.');
  if ('error' in target) {
    return `Error: ${target.error}`;
  }

  if (isForbiddenRelativePath(target.relativePath)) {
    return `Error: ${FORBIDDEN_DIRECT_MESSAGE}`;
  }

  const kind = await pathKind(options, target.absolutePath);
  if (kind === 'missing') {
    return `Error: path not found: ${target.displayPath}`;
  }

  if (kind !== 'directory') {
    return `Error: path is not a directory: ${target.displayPath}`;
  }

  const excludes = compileTreeExcludes(input.exclude ?? []);
  if ('error' in excludes) {
    return excludes.error;
  }

  const rootName = path.basename(target.absolutePath);
  if (excludes.patterns.some((pattern) => pattern.test(rootName))) {
    return `Error: exclude pattern matches the root directory: ${rootName}`;
  }

  const entries = await visibleEntries(
    options,
    target.absolutePath,
    excludes.patterns,
  );

  return renderTree(target.absolutePath, entries);
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
