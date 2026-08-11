import {
  client,
  methods,
  PROTOCOL_VERSION,
  type PromptResponse,
  type SessionUpdate,
} from '@agentclientprotocol/sdk';
import assert from 'node:assert/strict';
import { mkdtemp, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { ProviderErrorObject } from 'llms';

import { acp, sessionUpdate } from '../src/acp.js';
import type { RunRequest, Runner } from '../src/run.js';

test('ACP consumes the credential when constructing its built-in runner', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-acp-'));
  const path = join(root, 'credential');
  const sensitive = Object.entries(process.env).filter(([name]) =>
    /(?:api[_-]?key|token|secret|password|credentials?|access[_-]?key)/iu.test(
      name,
    ),
  );

  try {
    await writeFile(path, 'file-secret', { mode: 0o600 });
    process.env.OPENROUTER_API_KEY_FILE = path;

    acp({ mode: 'direct' });

    await assert.rejects(stat(path));
    assert.equal(process.env.OPENROUTER_API_KEY_FILE, undefined);
  } finally {
    for (const name of Object.keys(process.env)) {
      if (
        /(?:api[_-]?key|token|secret|password|credentials?|access[_-]?key)/iu.test(
          name,
        )
      ) {
        delete process.env[name];
      }
    }
    for (const [name, value] of sensitive) process.env[name] = value;
    await rm(root, { recursive: true, force: true });
  }
});

test('ACP runs a fresh runner per prompt and forwards normalized updates', async () => {
  const requests: RunRequest[] = [];
  const updates: SessionUpdate[] = [];
  let runners = 0;
  const application = acp({
    mode: 'direct',
    randomUUID: () => 'session-1',
    createRunner: () => {
      runners += 1;
      return {
        run: async (request, emit) => {
          requests.push(request);
          await emit({ type: 'status', status: 'running' });
          await emit({ type: 'message_delta', delta: 'answer' });
          await emit({
            type: 'tool_started',
            callId: 'call-1',
            name: 'terminal',
            input: { command: 'pwd' },
          });
          await emit({
            type: 'tool_completed',
            callId: 'call-1',
            name: 'terminal',
            output: { exit_code: 0 },
          });
        },
      };
    },
  });
  const consumer = client({ name: 'test-client' }).onNotification(
    methods.client.session.update,
    ({ params }) => {
      updates.push(params.update);
    },
  );

  await consumer.connectWith(application, async (context) => {
    const initialized = await context.request(methods.agent.initialize, {
      protocolVersion: PROTOCOL_VERSION,
    });
    assert.equal(initialized.protocolVersion, PROTOCOL_VERSION);
    assert.equal(initialized.agentInfo?.version, '0.1.8');

    const created = await context.request(methods.agent.session.new, {
      cwd: '/workspace',
      mcpServers: [],
    });
    const prompt = [
      { type: 'text' as const, text: 'first ' },
      {
        type: 'resource_link' as const,
        name: 'ignored',
        uri: 'file:///ignored',
      },
      { type: 'text' as const, text: 'second' },
    ];
    const first = (await context.request(methods.agent.session.prompt, {
      sessionId: created.sessionId,
      prompt,
    })) as PromptResponse;
    const second = (await context.request(methods.agent.session.prompt, {
      sessionId: created.sessionId,
      prompt: [{ type: 'text', text: 'again' }],
    })) as PromptResponse;

    assert.equal(first.stopReason, 'end_turn');
    assert.equal(second.stopReason, 'end_turn');
  });

  assert.equal(runners, 2);
  assert.equal(requests[0]?.prompt, 'first second');
  assert.equal(requests[0]?.cwd, '/workspace');
  assert.equal(
    updates.some(({ sessionUpdate }) => sessionUpdate === 'plan'),
    true,
  );
  assert.equal(
    updates.some(
      ({ sessionUpdate }) => sessionUpdate === 'agent_message_chunk',
    ),
    true,
  );
  assert.equal(
    updates.some(({ sessionUpdate }) => sessionUpdate === 'tool_call'),
    true,
  );
  assert.equal(
    updates.some(({ sessionUpdate }) => sessionUpdate === 'tool_call_update'),
    true,
  );
});

test('ACP cancellation aborts the session controller and returns cancelled', async () => {
  let started!: () => void;
  const running = new Promise<void>((resolve) => {
    started = resolve;
  });
  const runner: Runner = {
    run: async ({ signal }) => {
      started();
      await new Promise<void>((resolve) => {
        if (signal?.aborted === true) {
          resolve();
          return;
        }
        signal?.addEventListener('abort', () => resolve(), { once: true });
      });
    },
  };
  const application = acp({
    mode: 'mosaic',
    randomUUID: () => 'session-cancel',
    createRunner: () => runner,
  });

  await client({ name: 'test-client' }).connectWith(
    application,
    async (context) => {
      await context.request(methods.agent.initialize, {
        protocolVersion: PROTOCOL_VERSION,
      });
      const created = await context.request(methods.agent.session.new, {
        cwd: '/workspace',
        mcpServers: [],
      });
      const response = context.request(methods.agent.session.prompt, {
        sessionId: created.sessionId,
        prompt: [{ type: 'text', text: 'wait' }],
      });

      await running;
      await context.notify(methods.agent.session.cancel, {
        sessionId: created.sessionId,
      });

      assert.equal((await response).stopReason, 'cancelled');
    },
  );
});

test('ACP reports an authentic provider error code without its diagnostic message', async () => {
  const failure = new ProviderErrorObject({
    provider: 'unified',
    code: 'unsupported_model_feature',
    message: 'sensitive provider diagnostic',
  });
  const application = acp({
    mode: 'direct',
    randomUUID: () => 'session-failure',
    createRunner: () => ({
      run: async () => {
        throw failure;
      },
    }),
  });
  const write = process.stderr.write;
  let output = '';
  process.stderr.write = ((chunk: string | Uint8Array) => {
    output += chunk.toString();
    return true;
  }) as typeof process.stderr.write;

  try {
    await client({ name: 'test-client' }).connectWith(
      application,
      async (context) => {
        await context.request(methods.agent.initialize, {
          protocolVersion: PROTOCOL_VERSION,
        });
        const created = await context.request(methods.agent.session.new, {
          cwd: '/workspace',
          mcpServers: [],
        });

        await assert.rejects(
          context.request(methods.agent.session.prompt, {
            sessionId: created.sessionId,
            prompt: [{ type: 'text', text: 'fail' }],
          }),
        );
      },
    );
  } finally {
    process.stderr.write = write;
  }

  assert.equal(
    output,
    'MOSAIC benchmark prompt failed: unsupported_model_feature.\n',
  );
  assert.doesNotMatch(output, /sensitive provider diagnostic/u);
});

test('ACP mapping represents status and each tool terminal state', () => {
  assert.equal(
    sessionUpdate({ type: 'status', status: 'running' }).sessionUpdate,
    'plan',
  );
  assert.deepEqual(
    sessionUpdate({ type: 'tool_failed', callId: 'call-1', name: 'terminal' }),
    {
      sessionUpdate: 'tool_call_update',
      toolCallId: 'call-1',
      status: 'failed',
    },
  );
});
