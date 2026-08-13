import { createHash } from 'node:crypto';

import { planningCases } from './cases/index.js';
import type { CompositionClass, PlanningDomain } from './planning-schema.js';

export interface PlanningBenchmarkManifest {
  readonly schemaVersion: 1;
  readonly benchmark: 'mosaic-p0-p1-controlled';
  readonly caseCount: 24;
  readonly domains: Readonly<Record<PlanningDomain, number>>;
  readonly compositionClasses: Readonly<Record<CompositionClass, number>>;
  readonly conditions: readonly ['no-hints', 'gold', 'retrieved', 'distractor'];
  readonly caseIds: readonly string[];
  readonly casesSha256: string;
}

/** Returns the closed 6-by-4 controlled planning benchmark identity. */
export const planningBenchmarkManifest = (): PlanningBenchmarkManifest => {
  const caseIds = planningCases.map(({ id }) => id);
  const domains = counts(
    ['documents-finance', 'software', 'artifacts', 'communications'] as const,
    planningCases.map(({ domain }) => domain),
  );
  const compositionClasses = counts(
    ['A', 'B', 'C', 'D', 'E', 'F'] as const,
    planningCases.map(({ compositionClass }) => compositionClass),
  );
  return {
    schemaVersion: 1,
    benchmark: 'mosaic-p0-p1-controlled',
    caseCount: 24,
    domains,
    compositionClasses,
    conditions: ['no-hints', 'gold', 'retrieved', 'distractor'],
    caseIds,
    casesSha256: createHash('sha256')
      .update(`mosaic-p0-p1-controlled-v1\n${canonicalJson(planningCases)}`)
      .digest('hex'),
  };
};

const counts = <Key extends string>(
  keys: readonly Key[],
  values: readonly Key[],
): Readonly<Record<Key, number>> =>
  Object.fromEntries(
    keys.map((key) => [key, values.filter((value) => value === key).length]),
  ) as Readonly<Record<Key, number>>;

const canonicalJson = (value: unknown): string => {
  if (value === null) return 'null';
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (typeof value === 'object') {
    const record = value as Readonly<Record<string, unknown>>;
    return `{${Object.keys(record)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`)
      .join(',')}}`;
  }
  const serialized = JSON.stringify(value);
  if (serialized === undefined) throw new TypeError('Value is not JSON data.');
  return serialized;
};
