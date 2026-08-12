import {
  agent,
  methods,
  ndJsonStream,
  PROTOCOL_VERSION,
  type AgentApp,
  type SessionUpdate,
} from '@agentclientprotocol/sdk';
import { randomUUID } from 'node:crypto';
import { Readable, Writable } from 'node:stream';
import { ProviderErrorObject } from 'llms';

import { direct } from './direct.js';
import { mosaic } from './mosaic.js';
import type { RunEvent, RunMode, Runner } from './run.js';

type Session = {
  readonly cwd: string;
  controller?: AbortController;
};

export interface AcpOptions {
  readonly mode: RunMode;
  readonly createRunner?: (mode: RunMode) => Runner | Promise<Runner>;
  readonly randomUUID?: () => string;
  readonly reportFailure?: (error: unknown) => void;
}

/** Creates the stable ACP application while keeping transport concerns outside. */
export const acp = (options: AcpOptions): AgentApp => {
  const sessions = new Map<string, Session>();
  const identifier = options.randomUUID ?? randomUUID;
  const builtIn =
    options.createRunner === undefined
      ? defaultRunner(options.mode)
      : undefined;
  const createRunner =
    options.createRunner ??
    (() => builtIn as Exclude<typeof builtIn, undefined>);
  const reportFailure = options.reportFailure ?? defaultFailureReporter;

  return agent({ name: 'mosaic-benchmark' })
    .onRequest(methods.agent.initialize, ({ params }) => ({
      protocolVersion:
        params.protocolVersion === PROTOCOL_VERSION
          ? params.protocolVersion
          : PROTOCOL_VERSION,
      agentCapabilities: { loadSession: false },
      agentInfo: { name: 'mosaic-benchmark', version: '0.1.11' },
    }))
    .onRequest(methods.agent.session.new, ({ params }) => {
      const sessionId = identifier();
      sessions.set(sessionId, { cwd: params.cwd });
      return { sessionId };
    })
    .onRequest(methods.agent.session.prompt, async (context) => {
      const { params } = context;
      const session = sessions.get(params.sessionId);
      if (session === undefined) throw new Error('ACP session was not found.');

      session.controller?.abort();
      const controller = new AbortController();
      session.controller = controller;
      const signal = AbortSignal.any([controller.signal, context.signal]);

      try {
        const runner = await createRunner(options.mode);
        await runner.run(
          {
            prompt: textPrompt(params.prompt),
            cwd: session.cwd,
            signal,
          },
          async (event) => {
            await context.client.notify(methods.client.session.update, {
              sessionId: params.sessionId,
              update: sessionUpdate(event),
            });
          },
        );
        return { stopReason: signal.aborted ? 'cancelled' : 'end_turn' };
      } catch (error) {
        if (signal.aborted) return { stopReason: 'cancelled' };
        reportFailure(error);
        throw error;
      } finally {
        if (session.controller === controller) session.controller = undefined;
      }
    })
    .onNotification(methods.agent.session.cancel, ({ params }) => {
      sessions.get(params.sessionId)?.controller?.abort();
    });
};

/** Serves stable ACP v1 over stdin/stdout NDJSON until the connection closes. */
export const serveAcp = async (options: AcpOptions): Promise<void> => {
  const stream = ndJsonStream(
    Writable.toWeb(process.stdout),
    Readable.toWeb(process.stdin),
  );
  const connection = acp(options).connect(stream);
  await connection.closed;
};

export const sessionUpdate = (event: RunEvent): SessionUpdate => {
  switch (event.type) {
    case 'message_delta':
      return {
        sessionUpdate: 'agent_message_chunk',
        content: { type: 'text', text: event.delta },
      };
    case 'tool_started':
      return {
        sessionUpdate: 'tool_call',
        toolCallId: event.callId,
        title: event.name,
        name: event.name,
        kind: event.name === 'terminal' ? 'execute' : 'other',
        status: 'in_progress',
        ...(event.input === undefined ? {} : { rawInput: event.input }),
      };
    case 'tool_completed':
      return {
        sessionUpdate: 'tool_call_update',
        toolCallId: event.callId,
        status: 'completed',
        ...(event.output === undefined ? {} : { rawOutput: event.output }),
      };
    case 'tool_failed':
      return {
        sessionUpdate: 'tool_call_update',
        toolCallId: event.callId,
        status: 'failed',
      };
    case 'status':
      return {
        sessionUpdate: 'plan',
        entries: [
          {
            content: event.status,
            priority: 'medium',
            status: terminalStatus(event.status) ? 'completed' : 'in_progress',
          },
        ],
      };
  }
};

const textPrompt = (
  blocks: readonly { readonly type: string; readonly text?: string }[],
): string =>
  blocks
    .filter(
      (block): block is { readonly type: 'text'; readonly text: string } =>
        block.type === 'text' && typeof block.text === 'string',
    )
    .map(({ text }) => text)
    .join('');

const terminalStatus = (status: string): boolean =>
  status === 'completed' || status === 'failed' || status === 'cancelled';

const defaultRunner = (mode: RunMode): Runner =>
  mode === 'direct' ? direct() : mosaic();

const defaultFailureReporter = (error: unknown): void => {
  const code =
    error instanceof ProviderErrorObject ? `: ${error.data.code}` : '';
  process.stderr.write(`MOSAIC benchmark prompt failed${code}.\n`);
};
