import { posix as path } from 'node:path';

import type { Sandbox } from 'sandbox';

import { escapeRegExp, type SafeTarget } from './safety.js';

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

type SearchInput = {
  readonly pattern: string;
  readonly literal?: boolean;
  readonly ignoreCase?: boolean;
};

const MAX_OUTPUT_BYTES = 50 * 1024;
const MAX_LINE_LENGTH = 500;

export const collectGrep = async (
  sandbox: Sandbox,
  target: Extract<SafeTarget, { readonly absolutePath: string }>,
  files: readonly string[],
  isSingleFile: boolean,
  regex: RegExp,
  glob: RegExp | undefined,
  context: number,
  limit: number,
): Promise<GrepOutput> => {
  const matches: GrepMatch[] = [];
  let totalBytes = 0;
  let truncated = false;
  let linesTruncated = false;

  for (const file of files) {
    const relative = isSingleFile
      ? path.basename(file)
      : path.relative(target.absolutePath, file);

    if (
      glob !== undefined &&
      !glob.test(relative) &&
      !glob.test(path.basename(file))
    ) {
      continue;
    }

    const text = await sandbox.readFile(file).catch(() => undefined);
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

export const compileSearch = (input: SearchInput): RegExp | string => {
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
