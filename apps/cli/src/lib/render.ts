import type { AgentExecutionEvent } from '@a2a-js/sdk/server';

import type { TextRedactor } from './redact.js';

export type CliIo = {
  readonly stdout: { write(text: string): unknown };
  readonly stderr: { write(text: string): unknown };
};

export type RuntimeSessionSummary = {
  readonly contextId: string;
  readonly state: string;
  readonly prompt: string;
};

export const writeEvent = (
  io: CliIo,
  event: AgentExecutionEvent,
  redactText: TextRedactor,
): void => {
  for (const line of eventLines(event)) {
    io.stdout.write(`${redactText(line)}\n`);
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
