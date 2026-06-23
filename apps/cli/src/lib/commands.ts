import { randomUUID } from 'node:crypto';

import type { AgentExecutionEvent } from '@a2a-js/sdk/server';

import { loadEnv, requiredEnv, type CliEnv } from './env.js';
import {
  createFollowupMessageParams,
  createInitialMessageParams,
  type FeatureOptions,
} from './messages.js';
import { askOpenQuestions, openQuestionsFromEvent, type Prompt } from './questions.js';
import {
  createJsonRpcClient,
  fetchAgentCard,
  JsonRpcCliError,
  type JsonRpcClient,
} from './rpc.js';
import {
  latestTaskIdFromEvent,
  writeEvent,
  writeSessionList,
  type CliIo,
  type RuntimeSessionSummary,
} from './render.js';
import { createTextRedactor } from './redact.js';

type SessionListResponse = {
  readonly sessions: readonly RuntimeSessionSummary[];
};

type ReplayCompleteEvent = {
  readonly kind: 'doric/replay-complete';
  readonly contextId: string;
};

type ConnectStreamEvent = AgentExecutionEvent | ReplayCompleteEvent;

export type CommandDependencies = {
  readonly io: CliIo;
  readonly fetch: typeof fetch;
  readonly prompt: Prompt;
  readonly rootDir?: string;
  readonly env?: NodeJS.ProcessEnv;
};

export type FeatureCommandOptions = {
  readonly prompt: string;
  readonly repo: string;
  readonly branch?: string;
};

const FOLLOWUP_PREFIX = 'Open question answers:\n';

export const runFeature = async (
  options: FeatureCommandOptions,
  dependencies: CommandDependencies,
): Promise<void> => {
  const env = await commandEnv(dependencies);
  const rpc = await rpcClient(env, dependencies.fetch);
  const redactText = createTextRedactor(env);
  const contextId = randomUUID();
  let latestTaskId: string | undefined;
  let params = createInitialMessageParams(env, {
    ...options,
    contextId,
  } satisfies FeatureOptions);

  dependencies.io.stdout.write(`${contextId}\n`);

  while (true) {
    const result = await consumeEvents(
      rpc.stream('message/stream', params),
      dependencies,
      redactText,
      latestTaskId,
    );
    latestTaskId = result.latestTaskId;

    const questions =
      result.finalEvent === undefined ? [] : openQuestionsFromEvent(result.finalEvent);

    if (questions.length === 0) {
      return;
    }

    if (latestTaskId === undefined) {
      throw new Error('Input is required, but the latest task ID is unknown.');
    }

    params = createFollowupMessageParams({
      contextId,
      taskId: latestTaskId,
      text: `${FOLLOWUP_PREFIX}${await askOpenQuestions(questions, dependencies.prompt)}`,
    });
  }
};

export const runList = async (
  dependencies: CommandDependencies,
): Promise<void> => {
  const env = await commandEnv(dependencies);
  const rpc = await rpcClient(env, dependencies.fetch);
  const { sessions } = await rpc.call<SessionListResponse>(
    'doric/sessions/list',
    {},
  );

  writeSessionList(dependencies.io, sessions);
};

export const runConnect = async (
  contextId: string,
  dependencies: CommandDependencies,
): Promise<void> => {
  const env = await commandEnv(dependencies);
  const rpc = await rpcClient(env, dependencies.fetch);
  const redactText = createTextRedactor(env);
  const answered = new Set<string>();
  let latestTaskId: string | undefined;
  let replayComplete = false;

  for await (const event of rpc.stream<ConnectStreamEvent>(
    'doric/sessions/connect',
    { contextId },
  )) {
    if (isReplayCompleteEvent(event)) {
      replayComplete = true;
      continue;
    }

    writeEvent(dependencies.io, event, redactText);
    latestTaskId = latestTaskIdFromEvent(event) ?? latestTaskId;

    const messageId =
      event.kind === 'status-update' ? event.status.message?.messageId : undefined;
    const questions = openQuestionsFromEvent(event);

    if (
      !replayComplete ||
      questions.length === 0 ||
      messageId === undefined ||
      answered.has(messageId)
    ) {
      continue;
    }

    if (latestTaskId === undefined) {
      throw new Error('Input is required, but the latest task ID is unknown.');
    }

    answered.add(messageId);

    await rpc.call('message/send', {
      ...createFollowupMessageParams({
        contextId,
        taskId: latestTaskId,
        text: `${FOLLOWUP_PREFIX}${await askOpenQuestions(questions, dependencies.prompt)}`,
        blocking: false,
      }),
    });
  }
};

export const runKill = async (
  contextId: string,
  dependencies: CommandDependencies,
): Promise<void> => {
  const env = await commandEnv(dependencies);
  const rpc = await rpcClient(env, dependencies.fetch);

  try {
    await rpc.call('doric/sessions/kill', { contextId });
    dependencies.io.stdout.write(`killed ${contextId}\n`);
  } catch (error) {
    if (error instanceof JsonRpcCliError && error.code === -32001) {
      dependencies.io.stdout.write(`not-found ${contextId}\n`);
      return;
    }

    throw error;
  }
};

const consumeEvents = async (
  stream: AsyncGenerator<AgentExecutionEvent, void>,
  dependencies: CommandDependencies,
  redactText: (text: string) => string,
  initialTaskId: string | undefined,
): Promise<{
  readonly latestTaskId: string | undefined;
  readonly finalEvent?: AgentExecutionEvent;
}> => {
  let latestTaskId = initialTaskId;
  let finalEvent: AgentExecutionEvent | undefined;

  for await (const event of stream) {
    writeEvent(dependencies.io, event, redactText);
    latestTaskId = latestTaskIdFromEvent(event) ?? latestTaskId;

    if (event.kind === 'status-update' && event.final) {
      finalEvent = event;
    }
  }

  return {
    latestTaskId,
    ...(finalEvent === undefined ? {} : { finalEvent }),
  };
};

const commandEnv = (dependencies: CommandDependencies): Promise<CliEnv> =>
  loadEnv(dependencies.rootDir, dependencies.env);

const isReplayCompleteEvent = (
  event: ConnectStreamEvent,
): event is ReplayCompleteEvent => event.kind === 'doric/replay-complete';

const rpcClient = async (
  env: CliEnv,
  fetchImpl: typeof fetch,
): Promise<JsonRpcClient> => {
  const card = await fetchAgentCard(requiredEnv(env, 'AGENT_CARD_URL'), fetchImpl);

  return createJsonRpcClient(card.url, fetchImpl);
};
