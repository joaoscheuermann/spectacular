import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import test from 'node:test';

import express, { type Express } from 'express';

import { defaultConfig } from '../src/lib/config.js';
import { createConfigRouter } from '../src/routes/config.js';
import { createSessionsRouter } from '../src/routes/sessions.js';

test('keeps the configuration schema at the root config route', async () => {
  let active = snapshot(1);
  const service = {
    current: () => ({ snapshot: active }),
    replace: async (configuration: typeof defaultConfig) => {
      active = { ...snapshot(2), configuration };
      return active;
    },
  };
  const app = express();
  app.use(express.json());
  app.use('/config', createConfigRouter(service as never));
  const host = await serve(app);

  try {
    const current = await fetch(`${host.url}/config`);
    assert.equal(current.status, 200);
    assert.deepEqual(await current.json(), active);

    const invalid = await fetch(`${host.url}/config`, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ ...defaultConfig, providers: [] }),
    });
    assert.equal(invalid.status, 422);
    assert.equal(
      ((await invalid.json()) as { error: { code: string } }).error.code,
      'invalid_config',
    );
  } finally {
    await host.close();
  }
});

test('creates prompt-free sessions and exposes detail prompts and events', async () => {
  let afterSequence: number | undefined;
  const service = {
    create: async () => session,
    find: async () => session,
    list: async () => ({ sessions: [session] }),
    prompt: async () => ({ status: 'accepted', promptId }) as const,
    terminate: async () => session,
    delete: async () => 'active' as const,
    ssh: async () => ({ status: 'pending' }) as const,
    events: async (_id: string, sequence: number) => {
      afterSequence = sequence;
      return { events: [event], lastSequence: 2 };
    },
  };
  const app = express();
  app.use(express.json());
  app.use('/sessions', createSessionsRouter(service as never));
  const host = await serve(app);

  try {
    const created = await fetch(`${host.url}/sessions`, { method: 'POST' });
    assert.equal(created.status, 202);
    assert.deepEqual(await created.json(), {
      ...session,
      ssh: { href: `/sessions/${session.id}/ssh` },
    });

    const listed = await fetch(`${host.url}/sessions`);
    const detail = await fetch(`${host.url}/sessions/${session.id}`);
    const detailBody = (await detail.json()) as Record<string, unknown>;
    assert.deepEqual(detailBody, session);
    assert.equal('result' in detailBody, false);
    assert.equal('prompt' in detailBody, false);
    assert.deepEqual(
      ((await listed.json()) as { sessions: unknown[] }).sessions[0],
      detailBody,
    );

    const prompted = await fetch(`${host.url}/sessions/${session.id}/prompt`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ prompt: 'inspect the sandbox' }),
    });
    assert.equal(prompted.status, 202);
    assert.deepEqual(await prompted.json(), { promptId });

    const events = await fetch(
      `${host.url}/sessions/${session.id}/events?afterSequence=1`,
    );
    assert.equal(events.status, 200);
    assert.equal(events.headers.get('cache-control'), 'no-store');
    assert.equal(afterSequence, 1);
    assert.deepEqual(await events.json(), {
      events: [event],
      lastSequence: 2,
    });

    await fetch(`${host.url}/sessions/${session.id}/events`);
    assert.equal(afterSequence, 0);
    assert.equal((await fetch(`${host.url}/mosaic/sessions`)).status, 404);
  } finally {
    await host.close();
  }
});

test('returns stable 4xx errors for invalid and inactive prompt requests', async () => {
  let promptStatus: 'inactive' | 'missing' = 'inactive';
  const service = {
    find: async () => undefined,
    prompt: async () => ({ status: promptStatus }),
    ssh: async () => ({ status: 'missing' }),
    events: async () => undefined,
    delete: async () => 'missing',
  };
  const app = express();
  app.use(express.json());
  app.use('/sessions', createSessionsRouter(service as never));
  const host = await serve(app);

  try {
    await expectError(
      `${host.url}/sessions/not-a-uuid/prompt`,
      400,
      'invalid_session_id',
      { method: 'POST' },
    );
    await expectError(
      `${host.url}/sessions/${session.id}/prompt`,
      422,
      'invalid_prompt',
      {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ prompt: ' ' }),
      },
    );
    await expectError(
      `${host.url}/sessions/${session.id}/prompt`,
      409,
      'session_inactive',
      {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ prompt: 'work' }),
      },
    );
    promptStatus = 'missing';
    await expectError(
      `${host.url}/sessions/${session.id}/prompt`,
      404,
      'session_not_found',
      {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ prompt: 'work' }),
      },
    );
    await expectError(
      `${host.url}/sessions/${session.id}/events?afterSequence=-1`,
      400,
      'invalid_event_cursor',
    );
    await expectError(`${host.url}/sessions?limit=101`, 400, 'invalid_page');
  } finally {
    await host.close();
  }
});

const session = {
  id: '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601',
  state: 'queued',
  configRevision: 1,
  lastSequence: 0,
  createdAt: new Date(0).toISOString(),
  updatedAt: new Date(0).toISOString(),
};
const promptId = '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1602';
const event = {
  sessionId: session.id,
  promptId,
  sequence: 2,
  type: 'reasoning.delta',
  event: { type: 'reasoning.delta', delta: 'reasoning' },
  createdAt: new Date(1000).toISOString(),
};

const snapshot = (revision: number) => ({
  configuration: defaultConfig,
  revision,
  updatedAt: new Date(revision * 1000).toISOString(),
});

const expectError = async (
  url: string,
  status: number,
  code: string,
  init?: RequestInit,
) => {
  const response = await fetch(url, init);
  assert.equal(response.status, status);
  assert.equal(
    ((await response.json()) as { error: { code: string } }).error.code,
    code,
  );
};

const serve = async (app: Express) => {
  const server = createServer(app);
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  assert.ok(address !== null && typeof address === 'object');
  return {
    url: `http://127.0.0.1:${address.port}`,
    close: () =>
      new Promise<void>((resolve, reject) =>
        server.close((error) =>
          error === undefined ? resolve() : reject(error),
        ),
      ),
  };
};
