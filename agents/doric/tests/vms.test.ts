import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import test from 'node:test';

import express from 'express';
import type {
  SandboxProvisionInput,
  SandboxProvider,
  SandboxRuntime,
} from 'sandbox';

import { createVmRegistry } from '../src/lib/vms.js';
import { createVmsRouter } from '../src/routes/vms.js';

const input: SandboxProvisionInput = {
  image: 'node:22-slim',
  root: '/workspace',
  resources: { cpuCount: 1, memoryMiB: 512, diskMiB: 4096 },
  network: { mode: 'disabled', ssh: false },
};

test('tracks provisioned VMs until disposal completes', async () => {
  let nextId = 1;
  const provider: SandboxProvider = {
    provision: async () => runtime(`vm-${String(nextId++)}`),
  };
  const registry = createVmRegistry('firecracker', provider);

  const first = await registry.provider.provision(input);
  const second = await registry.provider.provision(input);

  assert.deepEqual(registry.list(), [
    { id: 'vm-1', provider: 'firecracker' },
    { id: 'vm-2', provider: 'firecracker' },
  ]);

  await first.dispose();

  assert.deepEqual(registry.list(), [{ id: 'vm-2', provider: 'firecracker' }]);
  await second.dispose();
});

test('returns every running VM from the mounted router', async () => {
  const app = express();
  app.use(
    '/vms',
    createVmsRouter({
      list: () => [
        { id: 'vm-1', provider: 'firecracker' },
        { id: 'vm-2', provider: 'firecracker' },
      ],
    }),
  );
  const server = createServer(app);

  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });

  try {
    const address = server.address();
    assert.notEqual(address, null);
    assert.equal(typeof address, 'object');
    if (address === null || typeof address !== 'object') return;

    const response = await fetch(
      `http://127.0.0.1:${String(address.port)}/vms`,
    );

    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), [
      { id: 'vm-1', provider: 'firecracker' },
      { id: 'vm-2', provider: 'firecracker' },
    ]);
  } finally {
    await new Promise<void>((resolve, reject) =>
      server.close((cause) =>
        cause === undefined ? resolve() : reject(cause),
      ),
    );
  }
});

const runtime = (id: string): SandboxRuntime => ({
  id,
  exec: async () => ({
    exitCode: 0,
    stdout: '',
    stderr: '',
    stdoutBytes: new Uint8Array(),
    stderrBytes: new Uint8Array(),
  }),
  putFile: async () => undefined,
  getFile: async () => new Uint8Array(),
  ssh: async () => undefined,
  dispose: async () => undefined,
});
