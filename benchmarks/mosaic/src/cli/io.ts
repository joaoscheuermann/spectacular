import { readFile, writeFile } from 'node:fs/promises';

import { artifactHash, canonicalJson } from '../core/index.js';

const readStdin = async (): Promise<string> => {
  const chunks: Buffer[] = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(String(chunk)));
  }
  return Buffer.concat(chunks).toString('utf8');
};

export const readJson = async (path: string): Promise<unknown> => {
  const text = path === '-' ? await readStdin() : await readFile(path, 'utf8');
  if (text.trim().length === 0) throw new TypeError('JSON input is empty');
  return JSON.parse(text) as unknown;
};

export const writeJsonExclusive = async (
  path: string,
  value: unknown,
): Promise<{ readonly path: string; readonly hash: string }> => {
  const content = `${canonicalJson(value)}\n`;
  await writeFile(path, content, { encoding: 'utf8', flag: 'wx', mode: 0o600 });
  return { path, hash: artifactHash(value) };
};

export const writeTextExclusive = async (
  path: string,
  content: string,
): Promise<{ readonly path: string; readonly hash: string }> => {
  await writeFile(path, content, { encoding: 'utf8', flag: 'wx', mode: 0o600 });
  return { path, hash: artifactHash(content) };
};

export const jsonLine = (value: unknown): string => `${canonicalJson(value)}\n`;

export const progress = (message: string): void => {
  process.stderr.write(`${message}\n`);
};

export const objectValue = (
  value: unknown,
  label = 'input',
): Readonly<Record<string, unknown>> => {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new TypeError(`${label} must be a JSON object`);
  }
  return value as Readonly<Record<string, unknown>>;
};

export const stringValue = (value: unknown, label: string): string => {
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new TypeError(`${label} must be a non-empty string`);
  }
  return value;
};

export const stringArray = (
  value: unknown,
  label: string,
): readonly string[] => {
  if (
    !Array.isArray(value) ||
    value.some((entry) => typeof entry !== 'string')
  ) {
    throw new TypeError(`${label} must be a string array`);
  }
  return value;
};
