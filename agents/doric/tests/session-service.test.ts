import assert from 'node:assert/strict';
import test from 'node:test';

import { defaultConfig } from '../src/lib/config.js';
import { createSessionService } from '../src/lib/session-service.js';
import type { Session, SessionState } from '../src/lib/sessions.js';

test('cancels a queued sandbox acquisition without starting Mosaic', async () => {
  const harness = createHarness();
  let executed = false;
  const pool = {
    acquire: ({ signal }: { signal?: AbortSignal }) =>
      new Promise((_resolve, reject) =>
        signal?.addEventListener('abort', () =>
          reject(new DOMException('aborted', 'AbortError')),
        ),
      ),
  };
  const service = createSessionService({
    ...harness.dependencies,
    pool: pool as never,
    execute: async () => {
      executed = true;
    },
  });

  const created = await service.create('request');
  assert.equal(created.state, 'queued');
  assert.deepEqual(await service.ssh(created.id), { status: 'pending' });
  assert.equal((await service.terminate(created.id))?.state, 'cancelling');
  await service.dispose();
  assert.equal(executed, false);
  assert.equal(harness.current().state, 'cancelled');
  assert.deepEqual(await service.ssh(created.id), { status: 'expired' });
});

test('marks a queued session failed when sandbox acquisition is exhausted', async () => {
  let finished: () => void = () => undefined;
  const failed = new Promise<void>((resolve) => {
    finished = resolve;
  });
  const harness = createHarness((state) => {
    if (state === 'failed') finished();
  });
  const service = createSessionService({
    ...harness.dependencies,
    pool: {
      acquire: async () => {
        throw new Error('creation attempts exhausted');
      },
    } as never,
  });

  const created = await service.create('request');
  assert.equal(created.state, 'queued');
  await failed;
  assert.equal(harness.current().state, 'failed');
  await service.dispose();
  assert.deepEqual(await service.ssh(created.id), { status: 'expired' });
});

test('aborts a running execution and releases its sandbox lease', async () => {
  let released = 0;
  let started: () => void = () => undefined;
  const running = new Promise<void>((resolve) => {
    started = resolve;
  });
  const harness = createHarness((state) => {
    if (state === 'running') started();
  });
  const service = createSessionService({
    ...harness.dependencies,
    pool: {
      acquire: async () => ({
        sandbox: sandbox('vm-1'),
        release: async () => {
          released += 1;
        },
      }),
    } as never,
    execute: ({ signal }) =>
      new Promise((_resolve, reject) =>
        signal.addEventListener('abort', () =>
          reject(new DOMException('aborted', 'AbortError')),
        ),
      ),
  });

  const created = await service.create('request');
  await running;
  assert.deepEqual(await service.ssh(created.id), {
    status: 'ready',
    vmId: 'vm-1',
    ssh: access,
  });
  assert.deepEqual(await service.sshForVm('vm-1'), {
    sessionId: created.id,
    ssh: access,
  });
  assert.equal((await service.terminate(created.id))?.state, 'cancelling');
  await service.dispose();
  assert.equal(harness.current().state, 'cancelled');
  assert.equal(released, 1);
  assert.equal(await service.sshForVm('vm-1'), undefined);
});

test('keeps the generation captured at creation while configuration changes', async () => {
  const harness = createHarness();
  const oldGeneration = { snapshot, marker: 'old' };
  const newGeneration = {
    snapshot: { ...snapshot, revision: 2 },
    marker: 'new',
  };
  let active = oldGeneration;
  let releaseAcquire: () => void = () => undefined;
  const acquired = new Promise<void>((resolve) => {
    releaseAcquire = resolve;
  });
  let executed: unknown;
  let markExecuted: () => void = () => undefined;
  const execution = new Promise<void>((resolve) => {
    markExecuted = resolve;
  });
  const service = createSessionService({
    ...harness.dependencies,
    config: { current: () => active } as never,
    pool: {
      acquire: async () => {
        await acquired;
        return { sandbox: sandbox('vm-1'), release: async () => undefined };
      },
    } as never,
    execute: async ({ generation }) => {
      executed = generation;
      markExecuted();
      return { status: 'completed' };
    },
  });

  await service.create('request');
  active = newGeneration;
  releaseAcquire();
  await execution;
  await service.dispose();
  assert.equal(executed, oldGeneration);
});

test('returns original persisted events after the requested sequence', async () => {
  const events = [storedEvent(1), storedEvent(2)];
  const harness = createHarness();
  const service = createSessionService({
    ...harness.dependencies,
    store: {
      ...harness.store,
      eventsAfter: async (_id: string, sequence: number) =>
        events.filter((value) => value.sequence > sequence),
    } as never,
    pool: {} as never,
  });

  assert.deepEqual(await service.events(id, 1), {
    events: [events[1]!.event],
    lastSequence: 2,
  });
});

const createHarness = (
  onUpdate: (state: SessionState) => void = () => undefined,
) => {
  let current = session('queued');
  const update = (state: SessionState) => {
    current = session(state);
    onUpdate(state);
    return current;
  };
  const store = {
    create: async () => ({ session: current, snapshot }),
    markRunning: async () =>
      current.state === 'queued' ? update('running') : current,
    requestCancellation: async () =>
      terminal.has(current.state) ? current : update('cancelling'),
    finish: async (_id: string, target: 'completed' | 'failed' | 'cancelled') =>
      update(current.state === 'cancelling' ? 'cancelled' : target),
    list: async () => ({ sessions: [], nextCursor: undefined }),
    find: async () => ({ session: current, snapshot }),
    eventsAfter: async () => [],
    delete: async () => 'active' as const,
  };
  return {
    current: () => current,
    store,
    dependencies: {
      store: store as never,
      config: { current: () => ({ snapshot }) } as never,
      publisher: {
        event: () => undefined,
        updated: (value: Session) => onUpdate(value.state),
        deleted: () => undefined,
      },
      logger: { debug: () => undefined, error: () => undefined } as never,
    },
  };
};

const id = '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601';
const snapshot = {
  configuration: defaultConfig,
  revision: 1,
  updatedAt: new Date(0).toISOString(),
};
const terminal = new Set<SessionState>(['completed', 'failed', 'cancelled']);
const session = (state: SessionState): Session => ({
  id,
  prompt: 'request',
  state,
  configRevision: 1,
  result: null,
  lastSequence: 0,
  createdAt: new Date(0).toISOString(),
  updatedAt: new Date(0).toISOString(),
});

const access = {
  host: '127.0.0.1',
  port: 2200,
  username: 'root' as const,
  privateKey: 'private-key',
  knownHosts: '[127.0.0.1]:2200 ssh-ed25519 host-key',
  hostKeyFingerprint: 'SHA256:test',
};

const sandbox = (sandboxId: string) => ({
  id: sandboxId,
  ssh: async () => access,
});

const storedEvent = (sequence: number) => ({
  sessionId: id,
  sequence,
  type: 'stage.started',
  event: {
    schemaVersion: 2,
    runId: id,
    sequence,
    type: 'stage.started',
    stage: 'plan',
  },
  createdAt: new Date(sequence * 1000).toISOString(),
});
