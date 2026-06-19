#!/usr/bin/env node

import { randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { A2AClient } from '@a2a-js/sdk/client';

const ROOT_DIR = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const ENV_PATH = resolve(ROOT_DIR, '.env');
const REDACTED_KEYS = new Set([
  'authorization',
  'github_token',
  'secret',
  'token',
  'provider_token',
]);
const SECRET_KEY_PATTERN = /(?:authorization|secret|token)/iu;
const SECRET_ENV_KEY_PATTERN = /(?:AUTHORIZATION|SECRET|TOKEN)/iu;

const HELP = `Usage: npm run doric:spawn-agent

Sends Doric's initial A2A message to a running Doric agent server.

Configuration:
  DORIC_AGENT_CARD_URL       Required URL to the A2A Agent Card.
  DORIC_GITHUB_REPO_URL      Required GitHub repository URL to clone.
  DORIC_GITHUB_TOKEN         Required GitHub token for repository access.
  DORIC_PROVIDER_ID          Required provider id.
  DORIC_PROVIDER_TYPE        Required provider type.
  DORIC_PROVIDER_TOKEN       Required provider token.
  DORIC_MODEL_ID             Required model id.
  DORIC_MODEL_PROVIDER       Required provider id used by the model.
  DORIC_MODEL_NAME           Required provider model name.
  DORIC_MODEL_REASONING      Optional model reasoning setting.
  DORIC_MODEL_INTERNAL_KEY   Optional model internal key.
  DORIC_TASK_ID              Required task id.
  DORIC_TASK_MODEL           Required model id used by the task.
  DORIC_CONTEXT_ID           Optional A2A context id.
  DORIC_INITIAL_PROMPT       Required user prompt sent after config.

The script redacts token-like fields when logging responses.`;

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

  return [key, unquote(key, value)];
};

const unquote = (key, value) => {
  if (value.startsWith('"') && value.endsWith('"')) {
    try {
      return JSON.parse(value);
    } catch {
      throw new Error(`${key} has an invalid quoted value.`);
    }
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
    throw new Error(`${key} is required. Set it in root .env or the shell.`);
  }

  return value;
};

const createTextRedactor = (env) => {
  const secrets = Object.entries(env)
    .filter(([key]) => SECRET_ENV_KEY_PATTERN.test(key))
    .map(([, value]) => optional(value))
    .filter((value) => value !== undefined && value.length >= 4);

  return (text) =>
    secrets.reduce(
      (redacted, secret) => redacted.replaceAll(secret, '[redacted]'),
      text,
    );
};

const createConfig = (env) => {
  const reasoning = optional(env.DORIC_MODEL_REASONING);
  const internalKey = optional(env.DORIC_MODEL_INTERNAL_KEY);

  return {
    github: {
      repo: { url: required(env, 'DORIC_GITHUB_REPO_URL') },
      token: required(env, 'DORIC_GITHUB_TOKEN'),
    },
    providers: [
      {
        id: required(env, 'DORIC_PROVIDER_ID'),
        type: required(env, 'DORIC_PROVIDER_TYPE'),
        token: required(env, 'DORIC_PROVIDER_TOKEN'),
      },
    ],
    models: [
      {
        id: required(env, 'DORIC_MODEL_ID'),
        provider: required(env, 'DORIC_MODEL_PROVIDER'),
        model: required(env, 'DORIC_MODEL_NAME'),
        ...(reasoning === undefined ? {} : { reasoning }),
        ...(internalKey === undefined ? {} : { internal_key: internalKey }),
      },
    ],
    tasks: [
      {
        id: required(env, 'DORIC_TASK_ID'),
        model: required(env, 'DORIC_TASK_MODEL'),
      },
    ],
  };
};

const createMessageParams = (env) => {
  const contextId = optional(env.DORIC_CONTEXT_ID);

  return {
    message: {
      kind: 'message',
      messageId: randomUUID(),
      role: 'user',
      ...(contextId === undefined ? {} : { contextId }),
      parts: [
        {
          kind: 'data',
          data: {
            type: 'config',
            data: createConfig(env),
          },
        },
        { kind: 'text', text: required(env, 'DORIC_INITIAL_PROMPT') },
      ],
    },
  };
};

const logJson = (type, value, redactText) => {
  console.log(
    JSON.stringify({ type, value: redact(value, redactText) }, null, 2),
  );
};

const redact = (value, redactText) =>
  redactValue(value, new WeakSet(), redactText);

const redactValue = (value, seen, redactText) => {
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

const isSecretKey = (key) =>
  REDACTED_KEYS.has(key.toLowerCase()) || SECRET_KEY_PATTERN.test(key);

const isRecord = (value) => typeof value === 'object' && value !== null;

const errorMessage = (error) =>
  error instanceof Error ? error.message : String(error);

const sendWithStreamingFallback = async (client, params, redactText) => {
  let streamed = false;

  try {
    for await (const event of client.sendMessageStream(params)) {
      streamed = true;
      logJson('stream-event', event, redactText);
    }

    return;
  } catch (error) {
    if (streamed) {
      throw error;
    }

    console.error(
      `Streaming unavailable; falling back to sendMessage: ${redactText(
        errorMessage(error),
      )}`,
    );
  }

  logJson(
    'send-message-response',
    await client.sendMessage(params),
    redactText,
  );
};

const run = async () => {
  const env = mergedEnv(parseEnv(await readEnvFile()));
  const redactText = createTextRedactor(env);

  try {
    const client = await A2AClient.fromCardUrl(
      required(env, 'DORIC_AGENT_CARD_URL'),
    );

    await sendWithStreamingFallback(
      client,
      createMessageParams(env),
      redactText,
    );
  } catch (error) {
    throw new Error(redactText(errorMessage(error)), { cause: error });
  }
};

if (hasHelpFlag(process.argv.slice(2))) {
  console.log(HELP);
} else {
  try {
    await run();
  } catch (error) {
    console.error(errorMessage(error));
    process.exitCode = 1;
  }
}
