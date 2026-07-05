import path from 'node:path';

import { normalizeRelativePath } from './paths.js';

export const extractConfigKeys = (
  relativePath: string,
  content: string,
): readonly string[] => {
  const basename = path.posix.basename(normalizeRelativePath(relativePath));
  const extension = path.posix.extname(basename).toLowerCase();

  if (extension === '.yaml' || extension === '.yml') {
    return topLevelYamlKeys(content);
  }

  if (extension === '.toml') {
    return topLevelTomlKeys(content);
  }

  const jsonKeys = objectKeys(parseJsonc(content));

  return jsonKeys.length > 0 ? jsonKeys : topLevelYamlKeys(content);
};

const parseJsonc = (content: string): Record<string, unknown> | undefined => {
  try {
    const value = JSON.parse(
      removeTrailingCommas(stripJsonComments(content)),
    ) as unknown;

    return isRecord(value) ? value : undefined;
  } catch {
    return undefined;
  }
};

const stripJsonComments = (content: string): string => {
  let result = '';
  let inString = false;
  let escaped = false;

  for (let index = 0; index < content.length; index += 1) {
    const char = content[index];
    const next = content[index + 1];

    if (inString) {
      const wasEscaped = escaped;
      result += char;
      escaped = char === '\\' ? !escaped : false;

      if (char === '"' && !wasEscaped) {
        inString = false;
      }

      continue;
    }

    if (char === '"') {
      inString = true;
      result += char;
      continue;
    }

    if (char === '/' && next === '/') {
      index = skipUntilLineEnd(content, index + 2);
      result += '\n';
      continue;
    }

    if (char === '/' && next === '*') {
      index = skipUntilBlockCommentEnd(content, index + 2);
      continue;
    }

    result += char;
  }

  return result;
};

const skipUntilLineEnd = (content: string, index: number): number => {
  let cursor = index;

  while (cursor < content.length && content[cursor] !== '\n') {
    cursor += 1;
  }

  return cursor;
};

const skipUntilBlockCommentEnd = (content: string, index: number): number => {
  let cursor = index;

  while (cursor < content.length - 1) {
    if (content[cursor] === '*' && content[cursor + 1] === '/') {
      return cursor + 1;
    }

    cursor += 1;
  }

  return cursor;
};

const removeTrailingCommas = (content: string): string =>
  content.replace(/,\s*([}\]])/gu, '$1');

const topLevelYamlKeys = (content: string): readonly string[] =>
  unique(
    content
      .split(/\r?\n/u)
      .map((line) => /^([A-Za-z0-9_.-]+)\s*:/u.exec(line))
      .map((match) => match?.[1])
      .filter((key): key is string => key !== undefined),
  );

const topLevelTomlKeys = (content: string): readonly string[] => {
  const keys: string[] = [];
  let inTable = false;

  for (const rawLine of content.split(/\r?\n/u)) {
    const line = stripTomlComment(rawLine).trim();

    if (line === '') {
      continue;
    }

    const table = /^\[\[?([A-Za-z0-9_.-]+)\]?\]$/u.exec(line);

    if (table !== null) {
      keys.push(table[1].split('.')[0] ?? table[1]);
      inTable = true;
      continue;
    }

    const key = /^([A-Za-z0-9_.-]+)\s*=/u.exec(line);

    if (key !== null && !inTable) {
      keys.push(key[1]);
    }
  }

  return unique(keys);
};

const stripTomlComment = (line: string): string => {
  let inString = false;
  let quote = '';

  for (let index = 0; index < line.length; index += 1) {
    const char = line[index];

    if ((char === '"' || char === "'") && (quote === '' || quote === char)) {
      inString = !inString;
      quote = inString ? char : '';
      continue;
    }

    if (char === '#' && !inString) {
      return line.slice(0, index);
    }
  }

  return line;
};

const objectKeys = (
  record: Record<string, unknown> | undefined,
): readonly string[] => Object.keys(record ?? {});

const unique = (values: readonly string[]): readonly string[] => [
  ...new Set(values),
];

const isRecord = (value: unknown): value is Record<string, unknown> =>
  value !== null && typeof value === 'object' && !Array.isArray(value);
