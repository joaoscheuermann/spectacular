import assert from 'node:assert/strict';
import { mkdtemp, mkdir, realpath, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';

import { createTerminal } from '../src/terminal.js';

const createRoot = (): Promise<string> =>
  mkdtemp(join(tmpdir(), 'mosaic-terminal-'));

test('uses the session directory and captures stdout, stderr, and exit codes', async () => {
  const root = await createRoot();
  try {
    const terminal = createTerminal({ cwd: root });
    const result = await terminal.execute({
      command: 'printf output; printf error >&2; exit 7',
    });

    assert.deepEqual(result, {
      exit_code: 7,
      stdout: 'output',
      stderr: 'error',
      timed_out: false,
      truncated: false,
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('resolves and validates the requested working directory', async () => {
  const root = await createRoot();
  try {
    const nested = join(root, 'nested');
    await mkdir(nested);
    const terminal = createTerminal({ cwd: root });
    const result = await terminal.execute({
      command: 'pwd',
      working_directory: 'nested',
    });

    assert.equal(result.stdout.trim(), await realpath(resolve(nested)));
    await assert.rejects(
      terminal.execute({ command: 'true', working_directory: 'missing' }),
      /Working directory/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('terminates commands that exceed the timeout', async () => {
  const root = await createRoot();
  try {
    const result = await createTerminal({ cwd: root }).execute({
      command: 'sleep 5',
      timeout_ms: 20,
    });

    assert.equal(result.exit_code, -1);
    assert.equal(result.timed_out, true);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('marks output as truncated after the compact output limit', async () => {
  const root = await createRoot();
  try {
    const result = await createTerminal({ cwd: root }).execute({
      command: 'i=0; while [ "$i" -lt 20000 ]; do printf x; i=$((i + 1)); done',
    });

    assert.equal(result.truncated, true);
    assert.ok(Buffer.byteLength(result.stdout) <= 12 * 1024);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('accepts a smaller per-call output limit', async () => {
  const root = await createRoot();
  try {
    const terminal = createTerminal({ cwd: root });
    const parsed = terminal.input.safeParse({
      command: 'i=0; while [ "$i" -lt 200 ]; do printf x; i=$((i + 1)); done',
      max_output_chars: 64,
    });

    assert.equal(parsed.success, true);
    if (!parsed.success) return;
    const result = await terminal.execute(parsed.data);
    assert.equal(result.truncated, true);
    assert.ok(result.stdout.length <= 64);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('cancels the detached process group when its signal aborts', async () => {
  const root = await createRoot();
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), 20);

  try {
    const terminal = createTerminal({ cwd: root, signal: controller.signal });
    const started = Date.now();
    await assert.rejects(
      terminal.execute({ command: "trap '' TERM; while :; do sleep 1; done" }),
      (error: unknown) => error instanceof Error && error.name === 'AbortError',
    );
    assert.ok(Date.now() - started < 1_000);
  } finally {
    clearTimeout(timer);
    await rm(root, { recursive: true, force: true });
  }
});

test('does not expose sensitive parent environment variables to commands', async () => {
  const root = await createRoot();
  const secrets = {
    OPENROUTER_API_KEY: 'openrouter-test-key',
    OPENROUTER_API_KEY_FILE: '/tmp/openrouter-test-credential',
    BENCHFLOW_PROVIDER_API_KEY: 'benchflow-test-key',
    BENCHFLOW_LITELLM_MASTER_KEY: 'benchflow-master-key',
    TEST_TOKEN: 'test-token',
    TEST_SECRET: 'test-secret',
    TEST_PASSWORD: 'test-password',
    TEST_CREDENTIALS: 'test-credentials',
    TEST_ACCESS_KEY: 'test-access-key',
  };
  const previous = Object.fromEntries(
    Object.keys(secrets).map((name) => [name, process.env[name]]),
  );

  try {
    Object.assign(process.env, secrets);
    const result = await createTerminal({ cwd: root }).execute({
      command: 'env',
    });

    for (const name of Object.keys(secrets)) {
      assert.doesNotMatch(result.stdout, new RegExp(`^${name}=`, 'mu'));
    }
  } finally {
    for (const name of Object.keys(secrets)) {
      const value = previous[name];
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
    await rm(root, { recursive: true, force: true });
  }
});
