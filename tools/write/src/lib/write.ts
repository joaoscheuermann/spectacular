import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';

import { createTool as defineTool } from 'tools';
import { z } from 'zod';

const description =
  "Write content to a file. Creates the file if it doesn't exist, overwrites if it does. Automatically creates parent directories.";

export const schema = z
  .object({
    path: z.string(),
    content: z.string(),
  })
  .strict();

export type WriteOutput = {
  readonly success: boolean;
  readonly bytes_written: number;
  readonly diff?: string;
  readonly error?: string;
};

type Options = {
  readonly workspaceRoot: string;
};

/** Creates the provider-neutral file write tool. */
export const createTool = ({ workspaceRoot }: Options) =>
  defineTool({
    name: 'write',
    description,
    schema,
    execute: (input): Promise<WriteOutput> => execute(workspaceRoot, input),
  });

const execute = async (
  workspaceRoot: string,
  input: z.output<typeof schema>,
): Promise<WriteOutput> => {
  if (input.path === '') {
    return writeError('Path must not be empty');
  }

  const filePath = resolvePath(workspaceRoot, input.path);
  const parent = path.dirname(filePath);
  if (parent === '') {
    return writeError('Invalid path: no parent directory');
  }

  try {
    await mkdir(parent, { recursive: true });
  } catch (error) {
    return writeError(`Failed to create parent directories: ${message(error)}`);
  }

  const oldContent = await readFile(filePath, 'utf8').catch(() => '');

  try {
    await writeFile(filePath, input.content, 'utf8');
  } catch (error) {
    return writeError(`Failed to write file: ${message(error)}`);
  }

  const diff = diffPreview(oldContent, input.content);
  return {
    success: true,
    bytes_written: Buffer.byteLength(input.content),
    diff: `Edited ${input.path} (+${diff.added} -${diff.removed})\n${diff.lines}`,
  };
};

const writeError = (error: string): WriteOutput => ({
  success: false,
  bytes_written: 0,
  error,
});

const diffPreview = (
  oldContent: string,
  newContent: string,
): {
  readonly added: number;
  readonly removed: number;
  readonly lines: string;
} => {
  const oldLines = splitLines(oldContent);
  const newLines = splitLines(newContent);
  const prefix = commonPrefix(oldLines, newLines);
  const suffix = commonSuffix(oldLines.slice(prefix), newLines.slice(prefix));
  const oldChanged = oldLines.slice(prefix, oldLines.length - suffix);
  const newChanged = newLines.slice(prefix, newLines.length - suffix);
  const start = Math.max(prefix - 3, 0);
  const endOld = Math.min(oldLines.length, prefix + oldChanged.length + 3);
  const endNew = Math.min(newLines.length, prefix + newChanged.length + 3);
  const width = String(Math.max(endOld, endNew, 1)).length;
  const lines: string[] = [];

  for (let index = start; index < prefix; index += 1) {
    lines.push(formatLine(index + 1, ' ', oldLines[index] ?? '', width));
  }
  for (const [index, line] of oldChanged.entries()) {
    lines.push(formatLine(prefix + index + 1, '-', line, width));
  }
  for (const [index, line] of newChanged.entries()) {
    lines.push(formatLine(prefix + index + 1, '+', line, width));
  }
  for (let index = prefix + oldChanged.length; index < endOld; index += 1) {
    lines.push(formatLine(index + 1, ' ', oldLines[index] ?? '', width));
  }

  return {
    added: newChanged.length,
    removed: oldChanged.length,
    lines: lines.length === 0 ? '' : lines.join('\n'),
  };
};

const splitLines = (value: string): readonly string[] =>
  value === ''
    ? []
    : value.endsWith('\n')
      ? value.slice(0, -1).split(/\r?\n/)
      : value.split(/\r?\n/);

const commonPrefix = (
  left: readonly string[],
  right: readonly string[],
): number => {
  let index = 0;
  while (
    index < left.length &&
    index < right.length &&
    left[index] === right[index]
  ) {
    index += 1;
  }
  return index;
};

const commonSuffix = (
  left: readonly string[],
  right: readonly string[],
): number => {
  let count = 0;
  while (
    count < left.length &&
    count < right.length &&
    left[left.length - count - 1] === right[right.length - count - 1]
  ) {
    count += 1;
  }
  return count;
};

const formatLine = (
  line: number,
  marker: string,
  text: string,
  width: number,
): string => `${String(line).padStart(width, ' ')} ${marker}${text}`;

const resolvePath = (workspaceRoot: string, value: string): string =>
  path.normalize(
    path.isAbsolute(value) ? value : path.join(workspaceRoot, value),
  );

const message = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);
