#!/usr/bin/env node

import { readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  createFetchTransport,
  createLocalCallbackServer,
  createOpenAiOAuth,
  openBrowser,
} from 'oauth';

const ROOT_DIR = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const ENV_PATH = resolve(ROOT_DIR, '.env');
const DEFAULT_OUTPUT_KEY = 'OPENAI_AUTHORIZATION';
const DEFAULT_CALLBACK_PATH = '/callback';

const HELP = `Usage: npm run openai:oauth

Starts an OpenAI OAuth authorization-code flow in your browser and writes the
resulting authorization header to root .env.

Configuration:
  OPENAI_OAUTH_CLIENT_ID          Required OAuth client id.
  OPENAI_OAUTH_CLIENT_SECRET      Optional OAuth client secret.
  OPENAI_OAUTH_SCOPE              Optional OAuth scope override.
  OPENAI_OAUTH_CALLBACK_HOST      Optional callback host. Defaults to 127.0.0.1.
  OPENAI_OAUTH_CALLBACK_PORT      Optional callback port. Defaults to an ephemeral port.
  OPENAI_OAUTH_CALLBACK_PATH      Optional callback path. Defaults to /callback.
  OPENAI_OAUTH_OUTPUT_KEY         Optional .env output key. Defaults to OPENAI_AUTHORIZATION.

The script never prints the token or existing .env contents.`;

const hasHelpFlag = (args) =>
  args.some((arg) => arg === '--help' || arg === '-h');

const readEnvFile = async () => {
  try {
    return await readFile(ENV_PATH, 'utf8');
  } catch (error) {
    if (error?.code === 'ENOENT') {
      return '';
    }

    throw error;
  }
};

const parseEnv = (contents) =>
  Object.fromEntries(
    contents
      .split(/\r?\n/)
      .map(parseEnvLine)
      .filter((entry) => entry !== undefined),
  );

const parseEnvLine = (line) => {
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

  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
    return undefined;
  }

  return [key, unquote(value)];
};

const unquote = (value) => {
  if (value.startsWith('"') && value.endsWith('"')) {
    return JSON.parse(value);
  }

  if (value.startsWith("'") && value.endsWith("'")) {
    return value.slice(1, -1);
  }

  return value;
};

const mergedEnv = (fileEnv) => ({
  ...fileEnv,
  ...Object.fromEntries(
    Object.entries(process.env).filter(([, value]) => value !== undefined),
  ),
});

const optional = (value) =>
  value === undefined || value === '' ? undefined : value;

const required = (env, key) => {
  const value = optional(env[key]);

  if (value === undefined) {
    throw new Error(
      `${key} is required. Set it in the environment or root .env.`,
    );
  }

  return value;
};

const parsePort = (value) => {
  const raw = optional(value);

  if (raw === undefined) {
    return undefined;
  }

  const port = Number.parseInt(raw, 10);

  if (
    !Number.isInteger(port) ||
    port < 0 ||
    port > 65_535 ||
    String(port) !== raw
  ) {
    throw new Error(
      'OPENAI_OAUTH_CALLBACK_PORT must be an integer from 0 to 65535.',
    );
  }

  return port;
};

const assertOutputKey = (key) => {
  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
    throw new Error('OPENAI_OAUTH_OUTPUT_KEY must be a valid .env key.');
  }

  return key;
};

const envLinePattern = (key) =>
  new RegExp(`^\\s*(?:export\\s+)?${escapeRegExp(key)}\\s*=`);

const escapeRegExp = (value) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

const serializeEnvValue = (value) => JSON.stringify(value);

const writeEnvKey = async (contents, key, value) => {
  const linePattern = envLinePattern(key);
  const lineEnding = contents.includes('\r\n') ? '\r\n' : '\n';
  const lines = contents === '' ? [] : contents.split(/\r?\n/);
  const hasTrailingNewline = contents === '' || /\r?\n$/.test(contents);
  const nextLine = `${key}=${serializeEnvValue(value)}`;
  let replaced = false;

  const nextLines = lines.flatMap((line) => {
    if (!linePattern.test(line)) {
      return [line];
    }

    if (replaced) {
      return [];
    }

    replaced = true;
    return [nextLine];
  });

  if (!replaced) {
    if (nextLines.length === 0) {
      nextLines.push(nextLine);
    } else if (nextLines.at(-1) !== '') {
      nextLines.push(nextLine);
    } else {
      nextLines[nextLines.length - 1] = nextLine;
    }
  }

  const nextContents = `${nextLines.join(lineEnding)}${hasTrailingNewline ? lineEnding : ''}`;
  await writeFile(ENV_PATH, nextContents, 'utf8');
};

const tokenStore = () => {
  let record;

  return {
    async load() {
      return record;
    },

    async save(next) {
      record = next;
    },
  };
};

const errorMessage = (error) =>
  error instanceof Error ? error.message : String(error);

const authorize = async () => {
  const envContents = await readEnvFile();
  const env = mergedEnv(parseEnv(envContents));
  const outputKey = assertOutputKey(
    optional(env.OPENAI_OAUTH_OUTPUT_KEY) ?? DEFAULT_OUTPUT_KEY,
  );
  const clientId = required(env, 'OPENAI_OAUTH_CLIENT_ID');
  let callbackServer;

  try {
    callbackServer = await createLocalCallbackServer({
      host: optional(env.OPENAI_OAUTH_CALLBACK_HOST),
      port: parsePort(env.OPENAI_OAUTH_CALLBACK_PORT),
      path: optional(env.OPENAI_OAUTH_CALLBACK_PATH) ?? DEFAULT_CALLBACK_PATH,
    });

    const openai = createOpenAiOAuth({
      transport: createFetchTransport(),
      tokenStore: tokenStore(),
      clientId,
      clientSecret: optional(env.OPENAI_OAUTH_CLIENT_SECRET),
      scope: optional(env.OPENAI_OAUTH_SCOPE),
      redirectUri: callbackServer.redirectUri,
      browserOpener: openBrowser,
      callbackServer,
    });

    await openai.authorize();
    const credential = await openai.oauth();
    await writeEnvKey(await readEnvFile(), outputKey, credential.authorization);
    console.error(
      `Saved OpenAI authorization header to .env key ${outputKey}.`,
    );
  } finally {
    await callbackServer?.close();
  }
};

if (hasHelpFlag(process.argv.slice(2))) {
  console.log(HELP);
} else {
  try {
    await authorize();
  } catch (error) {
    console.error(errorMessage(error));
    process.exitCode = 1;
  }
}
