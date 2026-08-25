import type { JsonObject, JsonValue } from '../types/json.js';

export const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

export const isJsonValue = (value: unknown): value is JsonValue => {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  ) {
    return Number.isFinite(value) || typeof value !== 'number';
  }

  if (Array.isArray(value)) {
    return value.every(isJsonValue);
  }

  return (
    isRecord(value) && Object.values(value).every((child) => isJsonValue(child))
  );
};

export const asJsonObject = (value: unknown): JsonObject | undefined =>
  isRecord(value) && isJsonValue(value) ? value : undefined;

export const excerpt = (value: string, max = 300): string =>
  value.length <= max ? value : `${value.slice(0, max)}...`;
