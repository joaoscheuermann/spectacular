import { posix as path } from 'node:path';

import {
  compileGlob,
  isForbiddenRelativePath,
  TREE_SAFE_EXCLUDES,
} from './safety.js';
import type { Entry } from './sandbox-exploration.js';

export const compileTreeExcludes = (
  patterns: readonly string[],
): { readonly patterns: readonly RegExp[] } | { readonly error: string } => {
  const compiled: RegExp[] = [];

  for (const pattern of [...TREE_SAFE_EXCLUDES, ...patterns]) {
    const value = compileGlob(pattern);
    if (typeof value === 'string') {
      return {
        error: `Error: invalid exclude pattern '${pattern}': ${value}`,
      };
    }

    compiled.push(value);
  }

  return {
    patterns: compiled,
  };
};

export const renderTree = (root: string, entries: readonly Entry[]): string => {
  const children = groupEntries(entries);
  const lines = [root];
  appendTree(root, '', children, lines);

  return filterTree(`${lines.join('\n')}\n`);
};

const appendTree = (
  dir: string,
  prefix: string,
  children: ReadonlyMap<string, readonly Entry[]>,
  output: string[],
): void => {
  const entries = children.get(dir) ?? [];

  for (const [index, entry] of entries.entries()) {
    const isLast = index === entries.length - 1;
    const connector = isLast ? '`-- ' : '|-- ';
    const name = path.basename(entry.path);

    if (!entry.isDirectory) {
      output.push(`${prefix}${connector}${name}`);
      continue;
    }

    const mark = output.length;
    output.push(`${prefix}${connector}${name}/`);
    const childPrefix = isLast ? `${prefix}    ` : `${prefix}|   `;
    appendTree(entry.path, childPrefix, children, output);

    if (output.length === mark + 1) {
      output[mark] = `${output[mark]} (empty)`;
    }
  }
};

const groupEntries = (
  entries: readonly Entry[],
): ReadonlyMap<string, readonly Entry[]> => {
  const groups = new Map<string, Entry[]>();

  for (const entry of entries) {
    const parent = path.dirname(entry.path);
    groups.set(parent, [...(groups.get(parent) ?? []), entry]);
  }

  return new Map(
    [...groups.entries()].map(([parent, values]) => [parent, sorted(values)]),
  );
};

const sorted = (entries: readonly Entry[]): readonly Entry[] =>
  [...entries].sort((left, right) => {
    if (left.isDirectory !== right.isDirectory) {
      return left.isDirectory ? -1 : 1;
    }

    return path
      .basename(left.path)
      .localeCompare(path.basename(right.path), undefined, {
        sensitivity: 'accent',
      });
  });

const filterTree = (value: string): string => {
  const output = value
    .split(/\r?\n/)
    .filter((line) => !isForbiddenTreeLine(line));

  return output.join('\n');
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
