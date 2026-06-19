import assert from 'node:assert/strict';
import test from 'node:test';

import { A2AError } from '@a2a-js/sdk/server';
import { createSessionStore } from 'session';

import type { DoricSessionContext } from '../src/lib/executor.js';
import {
  createConfig,
  createDoricTestHarness,
  createEventBus,
  createRequestContext,
  createTextPart,
} from './fakes.js';

test('initializes a sandbox session and clones the configured repo on the first message', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });
  const eventBus = createEventBus();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
    }),
    eventBus,
  );

  assert.equal(harness.dockerCreateCount(), 1);
  assert.deepEqual(harness.sandboxOptions[0], {
    docker: harness.docker,
    image: 'node:slim',
    name: 'doric-context-1',
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
});

test('normalizes context IDs before using them as Docker container names', async () => {
  const harness = createDoricTestHarness();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'workspace:feature/one',
    }),
    createEventBus(),
  );

  assert.equal(harness.sandboxOptions[0]?.name, 'doric-workspace-feature-one');
});

test('reuses an existing context without requiring config on later messages', async () => {
  const harness = createDoricTestHarness();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
    }),
    createEventBus(),
  );
  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      metadata: null,
      parts: [createTextPart('continue')],
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
      config: createConfig({
        repoUrl: 'https://github.com/example/repo-a',
        token: 'token-a',
      }),
    }),
    createEventBus(),
  );
  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-b',
      config: createConfig({
        repoUrl: 'https://github.com/example/repo-b',
        token: 'token-b',
      }),
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
        metadata: null,
        parts: [createTextPart('missing config')],
      }),
      createEventBus(),
    ),
    (error: unknown) => {
      assert.ok(error instanceof A2AError);
      assert.equal(error.code, -32602);
      assert.deepEqual(error.data, {
        code: 'missing_configuration',
        path: 'message.metadata.configuration',
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
    }),
    createEventBus(),
  );
  const second = harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      metadata: null,
      parts: [createTextPart('continue')],
    }),
    createEventBus(),
  );

  await Promise.all([first, second]);

  assert.equal(harness.dockerCreateCount(), 1);
});

test('updates only stored config when later message includes configuration', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });
  const initialConfig = createConfig({
    repoUrl: 'https://github.com/example/initial',
    token: 'initial-token',
  });
  const updatedConfig = createConfig({
    repoUrl: 'https://github.com/example/updated',
    token: 'updated-token',
  });

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      config: initialConfig,
    }),
    createEventBus(),
  );
  const session = sessions.get('context-1');

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      config: updatedConfig,
      parts: [createTextPart('continue')],
    }),
    createEventBus(),
  );

  assert.equal(harness.dockerCreateCount(), 1);
  assert.equal(sessions.get('context-1'), session);
  assert.deepEqual(sessions.get('context-1')?.config, updatedConfig);
  assert.deepEqual(harness.sandboxes[0]?.clones, [
    {
      url: 'https://github.com/example/initial',
      auth: { kind: 'token', token: 'initial-token' },
    },
  ]);
  assert.deepEqual(sessions.get('context-1')?.repo, {
    path: '/workspace/repo',
    commit: 'abc123',
  });
});

test('rejects invalid later config and leaves stored config unchanged', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });
  const initialConfig = createConfig();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      config: initialConfig,
    }),
    createEventBus(),
  );

  await assert.rejects(
    harness.executor.execute(
      createRequestContext({
        contextId: 'context-1',
        metadata: {
          configuration: {
            ...initialConfig,
            models: [{ id: 'planning', provider: 'openai', model: 1 }],
          },
        },
        parts: [createTextPart('continue')],
      }),
      createEventBus(),
    ),
    (error: unknown) => {
      assert.ok(error instanceof A2AError);
      assert.equal(error.code, -32602);
      assert.deepEqual(error.data, {
        code: 'invalid_config_field',
        path: 'message.metadata.configuration.models[0].model',
      });

      return true;
    },
  );

  assert.deepEqual(sessions.get('context-1')?.config, initialConfig);
  assert.equal(harness.dockerCreateCount(), 1);
  assert.equal(harness.sandboxes[0]?.clones.length, 1);
});

test('uses SDK-generated context IDs as session keys', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });
  const requestContext = createRequestContext({});

  await harness.executor.execute(requestContext, createEventBus());

  assert.ok(sessions.get(requestContext.contextId));
  assert.equal(harness.sandboxes[0]?.clones.length, 1);
});

test('skips Git installation when Git is already available', async () => {
  const harness = createDoricTestHarness({ hasGit: true });

  await harness.executor.execute(createRequestContext(), createEventBus());

  assert.deepEqual(
    harness.sandboxes[0]?.execs.map((input) => input.cmd.join(' ')),
    ['sh -lc command -v git >/dev/null 2>&1'],
  );
});

test('installs Git and CA certificates when Git is missing', async () => {
  const harness = createDoricTestHarness({ hasGit: false });

  await harness.executor.execute(createRequestContext(), createEventBus());

  assert.deepEqual(
    harness.sandboxes[0]?.execs.map((input) => input.cmd.join(' ')),
    [
      'sh -lc command -v git >/dev/null 2>&1',
      'sh -lc apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*',
      'sh -lc command -v git >/dev/null 2>&1',
    ],
  );
});

test('does not save a session when Git setup fails', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({
    sessions,
    hasGit: false,
    gitInstallFailure: new Error('apt failed'),
  });

  await assert.rejects(
    harness.executor.execute(
      createRequestContext({
        contextId: 'context-1',
      }),
      createEventBus(),
    ),
    /git install failed: apt failed/u,
  );

  assert.equal(sessions.get('context-1'), undefined);
  assert.deepEqual(sessions.list(), []);
  assert.equal(harness.sandboxes[0]?.clones.length, 0);
});

test('does not save a session when cloning fails', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const cloneFailure = new Error('clone failed');
  const harness = createDoricTestHarness({ sessions, cloneFailure });

  await assert.rejects(
    harness.executor.execute(
      createRequestContext({
        contextId: 'context-1',
      }),
      createEventBus(),
    ),
    /clone failed/u,
  );

  assert.equal(sessions.get('context-1'), undefined);
  assert.deepEqual(sessions.list(), []);
});
