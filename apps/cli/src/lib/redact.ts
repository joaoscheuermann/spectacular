import { optionalEnv, type CliEnv } from './env.js';

const REDACTED_KEYS = new Set([
  'authorization',
  'github_token',
  'secret',
  'token',
  'provider_token',
]);
const SECRET_KEY_PATTERN = /(?:authorization|secret|token)/iu;
const SECRET_ENV_KEY_PATTERN = /(?:AUTHORIZATION|SECRET|TOKEN)/u;

export type TextRedactor = (text: string) => string;

export const createTextRedactor = (env: CliEnv): TextRedactor => {
  const secrets = Object.entries(env)
    .filter(([key]) => SECRET_ENV_KEY_PATTERN.test(key))
    .map(([, value]) => optionalEnv(value))
    .filter((value): value is string => value !== undefined && value.length >= 4);

  return (text) =>
    secrets.reduce(
      (redacted, secret) => redacted.replaceAll(secret, '[redacted]'),
      text,
    );
};

export const redact = (value: unknown, redactText: TextRedactor): unknown =>
  redactValue(value, new WeakSet<object>(), redactText);

const redactValue = (
  value: unknown,
  seen: WeakSet<object>,
  redactText: TextRedactor,
): unknown => {
  if (typeof value === 'string') {
    return redactText(value);
  }

  if (Array.isArray(value)) {
    return value.map((item) => redactValue(item, seen, redactText));
  }

  if (!isRecord(value)) {
    return value;
  }

  if (seen.has(value)) {
    return '[circular]';
  }

  seen.add(value);

  return Object.fromEntries(
    Object.entries(value).map(([key, child]) => [
      key,
      isSecretKey(key) ? '[redacted]' : redactValue(child, seen, redactText),
    ]),
  );
};

const isSecretKey = (key: string): boolean =>
  REDACTED_KEYS.has(key.toLowerCase()) || SECRET_KEY_PATTERN.test(key);

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;
