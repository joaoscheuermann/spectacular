# sandpool

`sandpool` is a process-local, warmed FIFO pool of `SandboxSession` capacity.
It owns acquisition, bounded creation retries, replacement, and disposal while
remaining independent of Docker and Firecracker.

## Use

```ts
import { createDockerClient } from 'docker';
import pino from 'pino';
import { createSandbox } from 'sandbox';
import { createSandpool } from 'sandpool';

const provider = createDockerClient();
const pool = createSandpool({
  minIdle: 1,
  maxSandboxes: 4,
  maxCreateAttempts: 3,
  logger: pino(),
  create: () =>
    createSandbox({
      provider,
      image: 'node:22-bookworm',
      resources: { cpuCount: 2, memoryMiB: 2048, diskMiB: 4096 },
      network: { mode: 'disabled' },
    }),
});

await pool.waitUntilHeated();
const lease = await pool.acquire();

try {
  await lease.sandbox.exec({ cmd: ['node', '--version'] });
} finally {
  await lease.release();
}

// Dispose the whole pool during application shutdown.
await pool.dispose();
```

Acquisitions are served in FIFO order. Releasing a lease destroys that sandbox
and the pool creates a replacement when needed; a released session is never
reused. `release` and `dispose` are idempotent.

The pool retries failed factory calls with bounded backoff. After
`maxCreateAttempts` consecutive failures, pending acquisition and heat waiters
reject; later demand begins a fresh attempt batch. Pass an `AbortSignal` to
`acquire` or `waitUntilHeated` to cancel a wait. `status()` exposes lifecycle
and capacity counts for health reporting.

The injected Pino logger is required. Operational logs contain lifecycle and
capacity metadata; the pool does not own provider configuration or persistent
session state.

## Development

```console
npx nx build sandpool
npx nx test sandpool
```

Run the lifecycle integration target with:

```console
npx nx run sandpool:e2e
```
