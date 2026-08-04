import assert from 'node:assert/strict';
import test from 'node:test';
import { createDockerClient } from 'docker';
import pino from 'pino';
import { createSandbox } from 'sandbox';
import { createSandpool } from 'sandpool';
const timeoutMs = 20_000;
test('warms, executes, replaces, and disposes Docker sandboxes', async () => {
    const docker = createDockerClient({ timeoutMs });
    await assertDockerDaemonReachable(docker);
    const pool = createSandpool({
        minIdle: 1,
        maxContainers: 1,
        logger: pino({ enabled: false }),
        create: () => createSandbox({
            docker,
            image: 'node:22-slim',
            network: { mode: 'disabled' },
            timeoutMs,
        }),
    });
    try {
        await pool.waitUntilHeated();
        const first = await pool.acquire();
        const result = await first.sandbox.exec({
            cmd: ['node', '-e', 'console.log("sandpool-ready")'],
            timeoutMs,
        });
        assertSucceeded(result);
        assert.equal(result.stdout.trim(), 'sandpool-ready');
        await first.release();
        await pool.waitUntilHeated();
        const second = await pool.acquire();
        assert.notEqual(second.sandbox.id, first.sandbox.id);
        await second.release();
    }
    finally {
        await pool.dispose();
    }
    assert.equal(pool.status().lifecycle, 'disposed');
    assert.equal(pool.status().total, 0);
});
const assertDockerDaemonReachable = async (docker) => {
    try {
        await docker.ping({ timeoutMs });
    }
    catch (cause) {
        throw new Error('Docker daemon is unreachable. Start Docker and rerun `npx nx run sandpool:e2e`.', { cause });
    }
};
const assertSucceeded = (result) => {
    assert.equal(result.exitCode, 0, `sandbox command failed: ${result.stderr || result.stdout}`.trim());
};
