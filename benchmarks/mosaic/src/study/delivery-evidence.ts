import type { Case } from '../schemas/index.js';
import { canonicalJson, type JsonValue } from '../core/json.js';

export interface DeliveryEvidence {
  readonly exact: boolean;
  readonly matchedRefs: ReadonlySet<string>;
}

const object = (
  value: JsonValue | undefined,
): Readonly<Record<string, JsonValue>> | undefined =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Readonly<Record<string, JsonValue>>)
    : undefined;

const pathValue = (
  value: Readonly<Record<string, JsonValue>>,
  path: readonly string[],
): JsonValue | undefined => {
  let current: JsonValue = value;
  for (const segment of path) {
    const record = object(current);
    if (record === undefined || !Object.hasOwn(record, segment)) {
      return undefined;
    }
    current = record[segment] as JsonValue;
  }
  return current;
};

const parseObject = (
  value: string,
): Readonly<Record<string, JsonValue>> | undefined => {
  try {
    return object(JSON.parse(value) as JsonValue);
  } catch {
    return undefined;
  }
};

const documents = (
  text: string,
): readonly Readonly<Record<string, JsonValue>>[] => {
  const trimmed = text.trim();
  const whole = trimmed.length === 0 ? undefined : parseObject(trimmed);
  const fenced = [
    ...text.matchAll(/(`{3,})json[ \t]*\r?\n([\s\S]*?)\r?\n\1/giu),
  ].flatMap((match) => {
    const parsed = parseObject(match[2] ?? '');
    return parsed === undefined ? [] : [parsed];
  });
  return [...(whole === undefined ? [] : [whole]), ...fenced];
};

const deliveryTexts = (outcome: JsonValue): readonly string[] => {
  const value = object(outcome);
  const delivery = object(value?.['delivery']);
  const markdown =
    typeof delivery?.['markdown'] === 'string' ? [delivery['markdown']] : [];
  const parts = Array.isArray(delivery?.['parts'])
    ? delivery['parts'].flatMap((part) => {
        const body = object(part);
        const partMarkdown =
          typeof body?.['markdown'] === 'string' ? [body['markdown']] : [];
        const artifacts = Array.isArray(body?.['artifacts'])
          ? body['artifacts'].flatMap((artifact) => {
              const item = object(artifact);
              return item?.['kind'] === 'inline' &&
                item['mime'] === 'application/json' &&
                typeof item['data'] === 'string'
                ? [item['data']]
                : [];
            })
          : [];
        return [...partMarkdown, ...artifacts];
      })
    : [];
  const goals = Array.isArray(value?.['goals'])
    ? value['goals'].flatMap((goal) => {
        const body = object(goal);
        return typeof body?.['output'] === 'string' ? [body['output']] : [];
      })
    : [];
  return [...markdown, ...parts, ...goals];
};

/** Matches a terminal outcome against exact JSON and criterion-addressable fields. */
export const evaluateDelivery = (
  benchmarkCase: Case,
  outcome: JsonValue,
): DeliveryEvidence => {
  const expected = benchmarkCase.gold.expectedDelivery;
  const candidates = deliveryTexts(outcome).flatMap(documents);
  const canonical = [...new Set(candidates.map(canonicalJson))];
  const exact =
    canonical.length === 1 && canonical[0] === canonicalJson(expected.document);
  const matchedRefs = new Set(
    expected.fields.flatMap((field) => {
      const expectedValue = pathValue(expected.document, field.path);
      const matched = candidates.some((candidate) => {
        const actual = pathValue(candidate, field.path);
        return (
          actual !== undefined &&
          expectedValue !== undefined &&
          canonicalJson(actual) === canonicalJson(expectedValue)
        );
      });
      return matched ? [field.id] : [];
    }),
  );
  return { exact, matchedRefs };
};

/** Renders the architecture-neutral canonical document expected from every condition. */
export const canonicalDelivery = (benchmarkCase: Case): string =>
  canonicalJson(benchmarkCase.gold.expectedDelivery.document);
