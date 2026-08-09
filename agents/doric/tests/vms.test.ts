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
  assert.deepEqual(registry.find('vm-1'), {
    id: 'vm-1',
    provider: 'firecracker',
  });

  await first.dispose();

  assert.deepEqual(registry.list(), [{ id: 'vm-2', provider: 'firecracker' }]);
  assert.equal(registry.find('vm-1'), undefined);
  await second.dispose();
});

test('returns running VMs and exposes SSH only for a leased VM', async () => {
  const vms = [
    { id: 'vm-1', provider: 'firecracker' as const },
    { id: 'vm-2', provider: 'firecracker' as const },
  ];
  const app = express();
  app.use(
    '/vms',
    createVmsRouter({
      list: () => vms,
      find: (id) => vms.find((vm) => vm.id === id),
      ssh: async (id) =>
        id === 'vm-1' ? { sessionId, ssh: access } : undefined,
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
    assert.deepEqual(await response.json(), vms);

    const ssh = await fetch(
      `http://127.0.0.1:${String(address.port)}/vms/vm-1/ssh`,
    );
    assert.equal(ssh.status, 200);
    assert.equal(ssh.headers.get('cache-control'), 'no-store');
    assert.deepEqual(await ssh.json(), {
      vm: vms[0],
      sessionId,
      ssh: access,
    });

    const idle = await fetch(
      `http://127.0.0.1:${String(address.port)}/vms/vm-2/ssh`,
    );
    assert.equal(idle.status, 409);

    const missing = await fetch(
      `http://127.0.0.1:${String(address.port)}/vms/missing/ssh`,
    );
    assert.equal(missing.status, 404);
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

const sessionId = '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601';
const access = {
  host: '127.0.0.1',
  port: 2200,
  username: 'root' as const,
  privateKey: 'private-key',
  knownHosts: '[127.0.0.1]:2200 ssh-ed25519 host-key',
  hostKeyFingerprint: 'SHA256:test',
};
