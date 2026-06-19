import assert from 'node:assert/strict';
import test from 'node:test';

import { A2AError } from '@a2a-js/sdk/server';
import { createSessionStore } from 'session';

import { createExecutor, type DoricSessionContext } from '../src/index.js';
import {
  createConfigPart,
  createDoricTestHarness,
  createEventBus,
  createRequestContext,
} from './fakes.js';

test('rejects execution when no session store is provided', async () => {
  const executor = createExecutor();
  const eventBus = createEventBus();

  await assert.rejects(
    executor.execute(
      createRequestContext({
        parts: [createConfigPart()],
      }),
      eventBus,
    ),
    /Doric executor requires a session store/u,
  );
  assert.deepEqual(eventBus.events, []);
  assert.equal(eventBus.finishedCount, 0);
});

test('initializes a sandbox session and clones the configured repo on the first message', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });
  const eventBus = createEventBus();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      parts: [createConfigPart()],
    }),
    eventBus,
  );

  assert.equal(harness.dockerCreateCount(), 1);
  assert.deepEqual(harness.sandboxOptions[0], {
    docker: harness.docker,
    image: 'node:slim',
    network: { mode: 'bridge' },
  });
  assert.deepEqual(harness.sandboxes[0]?.clones, [
    {
      url: 'https://github.com/example/repo',
      auth: { kind: 'token', token: 'github-token' },
    },
  ]);
  assert.deepEqual(sessions.get('context-1')?.repo, {
    path: '/workspace/repo',
    commit: 'abc123',
  });
  assert.equal(eventBus.finishedCount, 1);
  assert.equal(eventBus.events[0]?.kind, 'message');
});

test('reuses an existing context without requiring config on later messages', async () => {
  const harness = createDoricTestHarness();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      parts: [createConfigPart()],
    }),
    createEventBus(),
  );
  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      parts: [{ kind: 'text', text: 'continue' }],
    }),
    createEventBus(),
  );

  assert.equal(harness.dockerCreateCount(), 1);
  assert.equal(harness.sandboxes.length, 1);
  assert.equal(harness.sandboxes[0]?.clones.length, 1);
});

test('isolates sessions across context IDs', async () => {
  const harness = createDoricTestHarness();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-a',
      parts: [
        createConfigPart({
          repoUrl: 'https://github.com/example/repo-a',
          token: 'token-a',
        }),
      ],
    }),
    createEventBus(),
  );
  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-b',
      parts: [
        createConfigPart({
          repoUrl: 'https://github.com/example/repo-b',
          token: 'token-b',
        }),
      ],
    }),
    createEventBus(),
  );

  assert.equal(harness.dockerCreateCount(), 2);
  assert.deepEqual(
    harness.sandboxes.map((sandbox) => sandbox.clones[0]?.url),
    ['https://github.com/example/repo-a', 'https://github.com/example/repo-b'],
  );
});

test('rejects missing config when the context is unknown', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });

  await assert.rejects(
    harness.executor.execute(
      createRequestContext({
        contextId: 'context-1',
        parts: [{ kind: 'text', text: 'missing config' }],
      }),
      createEventBus(),
    ),
    (error: unknown) => {
      assert.ok(error instanceof A2AError);
      assert.equal(error.code, -32602);
      assert.deepEqual(error.data, {
        code: 'invalid_first_part_kind',
        path: 'message.parts[0].kind',
      });

      return true;
    },
  );
  assert.deepEqual(sessions.list(), []);
});

test('does not require config for an in-flight context', async () => {
  const harness = createDoricTestHarness();
  const first = harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      parts: [createConfigPart()],
    }),
    createEventBus(),
  );
  const second = harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      parts: [{ kind: 'text', text: 'continue' }],
    }),
    createEventBus(),
  );

  await Promise.all([first, second]);

  assert.equal(harness.dockerCreateCount(), 1);
});

test('uses SDK-generated context IDs as session keys', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });
  const requestContext = createRequestContext({
    parts: [createConfigPart()],
  });

  await harness.executor.execute(requestContext, createEventBus());

  assert.ok(sessions.get(requestContext.contextId));
  assert.equal(harness.sandboxes[0]?.clones.length, 1);
});

test('skips Git installation when Git is already available', async () => {
  const harness = createDoricTestHarness({ hasGit: true });

  await harness.executor.execute(
    createRequestContext({
      parts: [createConfigPart()],
    }),
    createEventBus(),
  );

  assert.deepEqual(
    harness.sandboxes[0]?.execs.map((input) => input.cmd.join(' ')),
    ['sh -lc command -v git >/dev/null 2>&1'],
  );
});

test('installs Git and CA certificates when Git is missing', async () => {
  const harness = createDoricTestHarness({ hasGit: false });

  await harness.executor.execute(
    createRequestContext({
      parts: [createConfigPart()],
    }),
    createEventBus(),
  );

  assert.deepEqual(
    harness.sandboxes[0]?.execs.map((input) => input.cmd.join(' ')),
    [
      'sh -lc command -v git >/dev/null 2>&1',
      'sh -lc apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*',
      'sh -lc command -v git >/dev/null 2>&1',
    ],
  );
});

test('does not save a session when cloning fails', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const cloneFailure = new Error('clone failed');
  const harness = createDoricTestHarness({ sessions, cloneFailure });

  await assert.rejects(
    harness.executor.execute(
      createRequestContext({
        contextId: 'context-1',
        parts: [createConfigPart()],
      }),
      createEventBus(),
    ),
    /clone failed/u,
  );

  assert.equal(sessions.get('context-1'), undefined);
  assert.deepEqual(sessions.list(), []);
  assert.equal(harness.sandboxes[0]?.disposed, true);
});
