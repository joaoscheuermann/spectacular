import { JsonlParseError } from '../classes/parse-error.js';
import type { JsonlValue } from '../types/jsonl.js';
import { isJsonlValue } from './value.js';

export const serializeLine = (value: JsonlValue): string => {
  const serialized = JSON.stringify(value);

  if (serialized === undefined) {
    throw new TypeError('JSONL values must be JSON serializable.');
  }

  return `${serialized}\n`;
};

export const parseLine = (
  path: string,
  lineNumber: number,
  line: string,
): JsonlValue => {
  try {
    const parsed: unknown = JSON.parse(line);

    if (!isJsonlValue(parsed)) {
      throw new TypeError('Parsed line is not a JSON value.');
    }

    return parsed;
  } catch (cause) {
    throw new JsonlParseError(path, lineNumber, line, cause);
  }
};
