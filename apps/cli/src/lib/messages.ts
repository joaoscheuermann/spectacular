import { randomUUID } from 'node:crypto';

import type { MessageSendParams } from '@a2a-js/sdk';

import { requiredEnv, type CliEnv } from './env.js';

export type FeatureOptions = {
  readonly prompt: string;
  readonly repo: string;
  readonly branch?: string;
  readonly contextId: string;
};

const TEST_PROVIDER = {
  id: 'codex',
  type: 'codex',
};
const TEST_MODEL = {
  id: 'default',
  provider: TEST_PROVIDER.id,
  model: 'gpt-5.5',
};
const TEST_TASK = {
  id: 'coding',
  model: TEST_MODEL.id,
};

export const createInitialMessageParams = (
  env: CliEnv,
  options: FeatureOptions,
): MessageSendParams => ({
  message: {
    kind: 'message',
    messageId: randomUUID(),
    role: 'user',
    contextId: options.contextId,
    metadata: { configuration: createConfig(env, options) },
    parts: [{ kind: 'text', text: options.prompt }],
  },
});

export const createFollowupMessageParams = ({
  contextId,
  taskId,
  text,
  blocking,
}: {
  readonly contextId: string;
  readonly taskId: string;
  readonly text: string;
  readonly blocking?: boolean;
}): MessageSendParams => ({
  message: {
    kind: 'message',
    messageId: randomUUID(),
    role: 'user',
    contextId,
    taskId,
    parts: [{ kind: 'text', text }],
  },
  ...(blocking === undefined ? {} : { configuration: { blocking } }),
});

const createConfig = (env: CliEnv, options: FeatureOptions) => ({
  github: {
    repo: {
      url: options.repo,
      ...(options.branch === undefined ? {} : { branch: options.branch }),
    },
    token: requiredEnv(env, 'GITHUB_TOKEN'),
  },
  providers: [
    {
      ...TEST_PROVIDER,
      token: requiredEnv(env, 'CODEX_AUTHORIZATION'),
    },
  ],
  models: [{ ...TEST_MODEL }],
  tasks: [{ ...TEST_TASK }],
});
