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
  const service = {
    create: async (prompt: string) => ({ ...session, prompt }),
    list: async (limit: number, cursor?: string) => ({
      sessions: [session],
      limit,
      cursor,
    }),
    terminate: async () => undefined,
    delete: async () => 'active' as const,
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
