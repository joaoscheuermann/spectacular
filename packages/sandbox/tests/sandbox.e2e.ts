import assert from 'node:assert/strict';
import test from 'node:test';

import { createDockerClient, DockerHttpError, type DockerClient } from 'docker';
import {
  createSandbox,
  type SandboxExecResult,
  type SandboxSession,
} from 'sandbox';

type RuntimeProbe = {
  readonly version: string;
  readonly cwd: string;
};

const sandboxImage = 'node:slim';
const dockerTimeoutMs = 20_000;
const commandTimeoutMs = 10_000;

test('executes commands and exchanges files inside a node slim container', async () => {
  const docker = createDockerClient({ timeoutMs: dockerTimeoutMs });
  let session: SandboxSession | undefined;

  await assertDockerDaemonReachable(docker);

  try {
    session = await createNodeSlimSandbox(docker);

    assert.equal(session.root, '/workspace');

    const runtime = parseRuntimeProbe(
      assertSucceeded(
        await session.exec({
          cmd: [
            'node',
            '-e',
            'console.log(JSON.stringify({ version: process.version, cwd: process.cwd() }))',
          ],
          timeoutMs: commandTimeoutMs,
        }),
        'node runtime probe failed',
      ).stdout,
    );

    assert.match(runtime.version, /^v\d+\./u);
    assert.equal(runtime.cwd, '/workspace');

    await session.writeFile('notes/message.txt', 'hello from node slim\n');

    assert.equal(
      await session.readFile('/workspace/notes/message.txt'),
      'hello from node slim\n',
    );

    const cwd = assertSucceeded(
      await session.exec({
        cmd: ['node', '-e', 'console.log(process.cwd())'],
        cwd: '/workspace/notes',
        timeoutMs: commandTimeoutMs,
      }),
      'custom cwd probe failed',
    );

    assert.equal(cwd.stdout.trim(), '/workspace/notes');
  } finally {
    await session?.dispose();
  }
});

const assertDockerDaemonReachable = async (
  docker: DockerClient,
): Promise<void> => {
  try {
    await docker.ping({ timeoutMs: dockerTimeoutMs });
  } catch (cause) {
    throw new Error(
      'Docker daemon is unreachable. Start Docker and rerun `npx nx run sandbox:e2e`.',
      { cause },
    );
  }
};

const createNodeSlimSandbox = async (
  docker: DockerClient,
): Promise<SandboxSession> => {
  try {
    return await createSandbox({
      docker,
      image: sandboxImage,
      timeoutMs: dockerTimeoutMs,
    });
  } catch (cause) {
    if (isUnavailableImage(cause)) {
      throw new Error(
        'Docker image `node:slim` is unavailable. Run `docker pull node:slim` before `npx nx run sandbox:e2e`.',
        { cause },
      );
    }

    throw cause;
  }
};

const isUnavailableImage = (cause: unknown): boolean =>
  cause instanceof DockerHttpError &&
  cause.status === 404 &&
  cause.path.startsWith('/containers/create');

const assertSucceeded = (
  result: SandboxExecResult,
  label: string,
): SandboxExecResult => {
  assert.equal(
    result.exitCode,
    0,
    `${label}: ${result.stderr || result.stdout}`.trim(),
  );

  return result;
};

const parseRuntimeProbe = (stdout: string): RuntimeProbe => {
  const parsed: unknown = JSON.parse(stdout.trim());

  if (
    !isRecord(parsed) ||
    typeof parsed.version !== 'string' ||
    typeof parsed.cwd !== 'string'
  ) {
    assert.fail(`Unexpected node runtime probe output: ${stdout}`);
  }

  return {
    version: parsed.version,
    cwd: parsed.cwd,
  };
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;
