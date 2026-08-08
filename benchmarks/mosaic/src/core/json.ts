export type JsonPrimitive = string | number | boolean | null;
export type JsonValue =
  | JsonPrimitive
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };

const normalize = (value: unknown, path: string): JsonValue => {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'boolean'
  ) {
    return value;
  }

  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new TypeError(`non-finite number at ${path}`);
    }
    return Object.is(value, -0) ? 0 : value;
  }

  if (Array.isArray(value)) {
    return value.map((entry, index) => normalize(entry, `${path}[${index}]`));
  }

  if (typeof value !== 'object' || value === undefined) {
    throw new TypeError(`non-JSON value at ${path}`);
  }

  const record = value as Record<string, unknown>;
  return Object.fromEntries(
    Object.keys(record)
      .sort()
      .map((key) => [key, normalize(record[key], `${path}.${key}`)]),
  );
};

/** Produces the canonical JSON representation used by every benchmark hash. */
export const canonicalJson = (value: unknown): string =>
  JSON.stringify(normalize(value, '$'));

/** Returns a detached JSON snapshot and rejects values outside the JSON domain. */
export const jsonSnapshot = <T extends JsonValue>(value: T): T =>
  JSON.parse(canonicalJson(value)) as T;
