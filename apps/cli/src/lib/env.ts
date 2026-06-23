import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export type CliEnv = Readonly<Record<string, string | undefined>>;

const ROOT_FROM_DIST = '../../../..';

export const defaultRootDir = (): string =>
  resolve(dirname(fileURLToPath(import.meta.url)), ROOT_FROM_DIST);

/** Loads root .env and overlays shell environment values over it. */
export const loadEnv = async (
  rootDir = defaultRootDir(),
  shellEnv: NodeJS.ProcessEnv = process.env,
): Promise<CliEnv> => ({
  ...parseEnv(await readEnvFile(resolve(rootDir, '.env'))),
  ...Object.fromEntries(
    Object.entries(shellEnv).filter((entry): entry is [string, string] => entry[1] !== undefined),
  ),
});

export const parseEnv = (contents: string): CliEnv =>
  Object.fromEntries(
    contents
      .split(/\r?\n/u)
      .map(parseEnvLine)
      .filter((entry): entry is readonly [string, string] => entry !== undefined),
  );

export const requiredEnv = (env: CliEnv, key: string): string => {
  const value = optionalEnv(env[key]);

  if (value === undefined) {
    throw new Error(`${key} is required. Set it in root .env or the shell.`);
  }

  return value;
};

export const optionalEnv = (value: string | undefined): string | undefined =>
  value === undefined || value === '' ? undefined : value;

const readEnvFile = async (path: string): Promise<string> => {
  try {
    return await readFile(path, 'utf8');
  } catch (error) {
    if (isNodeError(error) && error.code === 'ENOENT') {
      return '';
    }

    throw error;
  }
};

const parseEnvLine = (line: string): readonly [string, string] | undefined => {
  const trimmed = line.trim();

  if (trimmed === '' || trimmed.startsWith('#')) {
    return undefined;
  }

  const normalized = trimmed.startsWith('export ')
    ? trimmed.slice('export '.length)
    : line;
  const separator = normalized.indexOf('=');

  if (separator < 1) {
    return undefined;
  }

  const key = normalized.slice(0, separator).trim();
  const value = normalized.slice(separator + 1).trim();

  if (!/^[A-Za-z_][A-Za-z0-9_]*$/u.test(key)) {
    return undefined;
  }

  return [key, unquote(key, value)];
};

const unquote = (key: string, value: string): string => {
  if (value.startsWith('"') && value.endsWith('"')) {
    try {
      return JSON.parse(value) as string;
    } catch {
      throw new Error(`${key} has an invalid quoted value.`);
    }
  }

  if (value.startsWith("'") && value.endsWith("'")) {
    return value.slice(1, -1);
  }

  return value;
};

const isNodeError = (error: unknown): error is NodeJS.ErrnoException =>
  error instanceof Error && 'code' in error;
