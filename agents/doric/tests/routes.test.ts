import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import test from 'node:test';

import express, { type Express } from 'express';

import { defaultConfig } from '../src/lib/config.js';
import { createConfigRouter } from '../src/routes/config.js';
import { createSessionsRouter } from '../src/routes/sessions.js';

test('serves and validates complete singleton configuration replacements', async () => {
  let revision = 1;
  let active = snapshot(revision);
  const service = {
    current: () => ({ snapshot: active }),
    replace: async (configuration: typeof defaultConfig) => {
      active = { ...snapshot(++revision), configuration };
      return active;
    },
  };
  const app = express();
  app.use(express.json());
  app.use('/mosaic/config', createConfigRouter(service as never));
  const host = await serve(app);

  try {
    const current = await fetch(`${host.url}/mosaic/config`);
    assert.equal(current.status, 200);
    assert.deepEqual(await current.json(), active);

    const invalid = await fetch(`${host.url}/mosaic/config`, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ ...defaultConfig, providers: [] }),
    });
    assert.equal(invalid.status, 422);
    assert.deepEqual(await invalid.json(), {
      error: {
        code: 'invalid_config',
        message: 'The Mosaic configuration is invalid.',
      },
    });

    const replaced = await fetch(`${host.url}/mosaic/config`, {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(defaultConfig),
    });
    assert.equal(replaced.status, 200);
    assert.equal(((await replaced.json()) as { revision: number }).revision, 2);
  } finally {
    await host.close();
  }
});

test('returns stable session status and error envelopes', async () => {
  let ssh: unknown = { status: 'pending' };
  let afterSequence: number | undefined;
  const service = {
    create: async (prompt: string) => ({ ...session, prompt }),
    list: async (limit: number, cursor?: string) => ({
      sessions: [session],
      limit,
      cursor,
    }),
    terminate: async () => undefined,
    delete: async () => 'active' as const,
    ssh: async () => ssh,
    events: async (_id: string, sequence: number) => {
      afterSequence = sequence;
      return { events: [event], lastSequence: 2 };
    },
  };
  const app = express();
  app.use(express.json());
  app.use('/mosaic/sessions', createSessionsRouter(service as never));
  const host = await serve(app);

  try {
    const created = await fetch(`${host.url}/mosaic/sessions`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ prompt: 'request' }),
    });
    assert.equal(created.status, 202);
    assert.deepEqual(await created.json(), {
      ...session,
      ssh: { href: `/mosaic/sessions/${session.id}/ssh` },
    });

    const pending = await fetch(
      `${host.url}/mosaic/sessions/${session.id}/ssh`,
    );
    assert.equal(pending.status, 202);
    assert.equal(pending.headers.get('retry-after'), '1');
    assert.equal(pending.headers.get('cache-control'), 'no-store');
    assert.deepEqual(await pending.json(), { status: 'pending' });

    ssh = { status: 'ready', vmId: 'vm-1', ssh: access };
    const ready = await fetch(`${host.url}/mosaic/sessions/${session.id}/ssh`);
    assert.equal(ready.status, 200);
    assert.deepEqual(await ready.json(), {
      status: 'ready',
      vmId: 'vm-1',
      href: '/vms/vm-1/ssh',
      ssh: access,
    });

    const events = await fetch(
      `${host.url}/mosaic/sessions/${session.id}/events?afterSequence=1`,
    );
    assert.equal(events.status, 200);
    assert.equal(events.headers.get('cache-control'), 'no-store');
    assert.equal(afterSequence, 1);
    assert.deepEqual(await events.json(), {
      events: [event],
      lastSequence: 2,
    });

    const allEvents = await fetch(
      `${host.url}/mosaic/sessions/${session.id}/events`,
    );
    assert.equal(allEvents.status, 200);
    assert.equal(afterSequence, 0);
    assert.deepEqual(await allEvents.json(), {
      events: [event],
      lastSequence: 2,
    });

    const invalidEvents = await fetch(
      `${host.url}/mosaic/sessions/${session.id}/events?afterSequence=-1`,
    );
    assert.equal(invalidEvents.status, 400);
    assert.equal(
      ((await invalidEvents.json()) as { error: { code: string } }).error.code,
      'invalid_event_cursor',
    );

    const invalidPage = await fetch(`${host.url}/mosaic/sessions?limit=101`);
    assert.equal(invalidPage.status, 400);
    assert.equal(
      ((await invalidPage.json()) as { error: { code: string } }).error.code,
      'invalid_page',
    );

    const active = await fetch(`${host.url}/mosaic/sessions/${session.id}`, {
      method: 'DELETE',
    });
    assert.equal(active.status, 409);
    assert.equal(
      ((await active.json()) as { error: { code: string } }).error.code,
      'session_active',
    );
  } finally {
    await host.close();
  }
});

test('maps unavailable SSH and missing event history to stable errors', async () => {
  let status = 'missing';
  const service = {
    ssh: async () => ({ status }),
    events: async () => undefined,
  };
  const app = express();
  app.use('/mosaic/sessions', createSessionsRouter(service as never));
  const host = await serve(app);

  try {
    const sshUrl = `${host.url}/mosaic/sessions/${session.id}/ssh`;
    await expectError(sshUrl, 404, 'session_not_found');
    status = 'unavailable';
    await expectError(sshUrl, 409, 'session_ssh_unavailable');
    status = 'expired';
    await expectError(sshUrl, 410, 'session_ssh_expired');
    await expectError(
      `${host.url}/mosaic/sessions/${session.id}/events`,
      404,
      'session_not_found',
    );
  } finally {
    await host.close();
  }
});

const session = {
  id: '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601',
  prompt: 'request',
  state: 'queued',
  configRevision: 1,
  result: null,
  lastSequence: 0,
  createdAt: new Date(0).toISOString(),
  updatedAt: new Date(0).toISOString(),
};

const snapshot = (revision: number) => ({
  configuration: defaultConfig,
  revision,
  updatedAt: new Date(revision * 1000).toISOString(),
});

const access = {
  host: '127.0.0.1',
  port: 2200,
  username: 'root',
  privateKey: 'private-key',
  knownHosts: '[127.0.0.1]:2200 ssh-ed25519 host-key',
  hostKeyFingerprint: 'SHA256:test',
};

const event = {
  schemaVersion: 2,
  runId: session.id,
  sequence: 2,
  type: 'stage.started',
  stage: 'plan',
};

const expectError = async (url: string, status: number, code: string) => {
  const response = await fetch(url);
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
