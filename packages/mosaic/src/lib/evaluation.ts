import type { EvaluationHook } from './types/evaluation.js';

/** Runs an optional evaluation hook against an immutable input snapshot. */
export const evaluate = async <Input, Output>(
  hook: EvaluationHook<Input, Output> | undefined,
  input: Input,
  next: (input: Readonly<Input>) => Promise<Output>,
): Promise<Output> => {
  const snapshot = readonlySnapshot(input);

  if (hook === undefined) {return next(snapshot);}

  return hook(snapshot, (candidate) =>
    next(readonlySnapshot(candidate as Input)),
  );
};

const readonlySnapshot = <Value>(value: Value): Readonly<Value> =>
  freezeCopy(value, new WeakMap<object, unknown>()) as Readonly<Value>;

const freezeCopy = (
  value: unknown,
  seen: WeakMap<object, unknown>,
): unknown => {
  if (typeof value !== 'object' || value === null) {return value;}

  if (
    !Array.isArray(value) &&
    Object.getPrototypeOf(value) !== Object.prototype
  ) {
    return value;
  }

  const existing = seen.get(value);

  if (existing !== undefined) {return existing;}

  if (Array.isArray(value)) {
    const copy: unknown[] = [];

    seen.set(value, copy);

    value.forEach((item) => copy.push(freezeCopy(item, seen)));

    return Object.freeze(copy);
  }

  const copy: Record<string, unknown> = {};

  seen.set(value, copy);

  Object.entries(value).forEach(([key, item]) => {
    copy[key] = freezeCopy(item, seen);
  });

  return Object.freeze(copy);
};
