import { Writable } from 'node:stream';

import type { AgentExecutionEvent } from '@a2a-js/sdk/server';
import pino, { type Logger } from 'pino';
import pretty from 'pino-pretty';

import { redact, type TextRedactor } from './redact.js';

export type CliIo = {
  readonly stdout: { readonly isTTY?: boolean; write(text: string): unknown };
  readonly stderr: { readonly isTTY?: boolean; write(text: string): unknown };
};

export type RuntimeSessionSummary = {
  readonly contextId: string;
  readonly state: string;
  readonly prompt: string;
};

export type EventWriter = (event: AgentExecutionEvent) => void;

type EventLogLevel = 'error' | 'info' | 'success' | 'warn';
type EventLogger = Logger<'success'>;

export const createEventWriter = (
  io: CliIo,
  redactText: TextRedactor,
): EventWriter => {
  const logger = createEventLogger(io);

  return (event) => writeEventToLogger(logger, event, redactText);
};

export const writeEvent = (
  io: CliIo,
  event: AgentExecutionEvent,
  redactText: TextRedactor,
): void => {
  createEventWriter(io, redactText)(event);
};

const writeEventToLogger = (
  logger: EventLogger,
  event: AgentExecutionEvent,
  redactText: TextRedactor,
): void => {
  const fields = redact(eventFields(event), redactText) as Record<
    string,
    unknown
  >;
  const lines = eventLines(event);
  const rendered = lines.length === 0 ? [event.kind] : lines;
  const level = eventLogLevel(event);

  for (const line of rendered) {
    logger[level](fields, redactText(line));
  }
};

export const writeSessionList = (
  io: Pick<CliIo, 'stdout'>,
  sessions: readonly RuntimeSessionSummary[],
): void => {
  for (const session of sessions) {
    io.stdout.write(
      `${session.contextId} ${session.state} ${singleLine(session.prompt)}\n`,
    );
  }
};

export const latestTaskIdFromEvent = (
  event: AgentExecutionEvent,
): string | undefined => {
  if (event.kind === 'task') {
    return event.id;
  }

  return event.taskId;
};

const createEventLogger = (io: CliIo): EventLogger => {
  const destination = new Writable({
    write(chunk, _encoding, callback) {
      io.stdout.write(chunk.toString('utf8'));
      callback();
    },
  });
  const stream = pretty({
    colorize: true,
    customColors: 'error:red,warn:yellow,info:blue,success:green',
    customLevels: { success: 35 },
    destination,
    ignore: 'pid,hostname',
    messageFormat: '{msg}',
    singleLine: true,
    sync: true,
    translateTime: true,
  });

  return pino<'success'>(
    {
      base: undefined,
      customLevels: { success: 35 },
      timestamp: true,
    },
    stream,
  );
};

const eventLogLevel = (event: AgentExecutionEvent): EventLogLevel => {
  const state = eventState(event);

  if (
    event.kind === 'status-update' &&
    event.final &&
    state === 'completed'
  ) {
    return 'success';
  }

  if (stringField(event, 'kind') === 'error' || isErrorState(state)) {
    return 'error';
  }

  if (isAttentionState(state)) {
    return 'warn';
  }

  return 'info';
};

const eventFields = (event: AgentExecutionEvent): Record<string, unknown> =>
  withoutUndefined({
    eventKind: event.kind,
    contextId: stringField(event, 'contextId'),
    taskId: latestTaskIdFromEvent(event),
    state: eventState(event),
    final: event.kind === 'status-update' ? event.final : undefined,
  });

const eventState = (event: AgentExecutionEvent): string | undefined =>
  event.kind === 'task' || event.kind === 'status-update'
    ? event.status.state
    : undefined;

const isErrorState = (state: string | undefined): boolean =>
  state === 'canceled' ||
  state === 'failed' ||
  state === 'rejected' ||
  state === 'error';

const isAttentionState = (state: string | undefined): boolean =>
  state === 'auth-required' || state === 'input-required' || state === 'unknown';

const withoutUndefined = (
  fields: Record<string, unknown>,
): Record<string, unknown> =>
  Object.fromEntries(
    Object.entries(fields).filter(([, value]) => value !== undefined),
  );

const stringField = (value: unknown, key: string): string | undefined =>
  isRecord(value) && typeof value[key] === 'string' ? value[key] : undefined;

const eventLines = (event: AgentExecutionEvent): readonly string[] => {
  if (event.kind === 'task') {
    return messageTextLines(event.status.message);
  }

  if (event.kind === 'status-update') {
    return messageTextLines(event.status.message);
  }

  if (event.kind === 'artifact-update') {
    return event.artifact.parts.flatMap((part) =>
      part.kind === 'text' ? [part.text] : [],
    );
  }

  return messageTextLines(event);
};

const messageTextLines = (
  message: { readonly parts?: unknown } | undefined,
): readonly string[] => {
  if (!isRecord(message) || !Array.isArray(message['parts'])) {
    return [];
  }

  return message['parts'].flatMap((part) =>
    isRecord(part) && part['kind'] === 'text' && typeof part['text'] === 'string'
      ? [part['text']]
      : [],
  );
};

const singleLine = (value: string): string => value.replaceAll(/\s+/gu, ' ').trim();

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;
