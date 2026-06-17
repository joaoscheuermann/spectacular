import type { JsonlValue } from '../types/jsonl.js';

export const isJsonlValue = (value: unknown): value is JsonlValue => {
  if (value === null) {
    return true;
  }

  if (typeof value === 'string' || typeof value === 'boolean') {
    return true;
  }

  if (typeof value === 'number') {
    return Number.isFinite(value);
  }

  if (Array.isArray(value)) {
    return value.every(isJsonlValue);
  }

  if (typeof value === 'object') {
    return Object.values(value).every(isJsonlValue);
  }

  return false;
};
