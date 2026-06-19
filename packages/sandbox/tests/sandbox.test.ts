import { Buffer } from 'node:buffer';
import assert from 'node:assert/strict';
import test from 'node:test';

import type {
  ArchiveReadInput,
  ArchiveWriteInput,
  ContainerRef,
  CreateContainerInput,
  DockerClient,
  DockerVersion,
  ExecInput,
  ExecResult,
  RemoveContainerOptions,
} from 'docker';

import { createSandbox } from '../src/index.js';

type FakeDocker = DockerClient & {
  readonly creates: CreateContainerInput[];
  readonly starts: string[];
  readonly execs: ExecInput[];
  readonly removes: RemoveContainerOptions[];
  readonly archives: Map<string, Uint8Array>;
  failStart: boolean;
  failRemove: boolean;
};

const ok = (stdout = ''): ExecResult => ({
  exitCode: 0,
  stdout,
  stderr: '',
  stdoutBytes: Buffer.from(stdout),
  stderrBytes: new Uint8Array(),
});

const fakeDocker = (): FakeDocker => {
  const archives = new Map<string, Uint8Array>();
  const docker: FakeDocker = {
    creates: [],
    starts: [],
    execs: [],
    removes: [],
    archives,
    failStart: false,
    failRemove: false,

    async ping() {
      return undefined;
    },

    async version(): Promise<DockerVersion> {
      return { raw: {} };
    },

    async createContainer(input: CreateContainerInput) {
      docker.creates.push(input);
      return { id: 'container-1', warnings: [] };
    },

    async startContainer(container: ContainerRef | string) {
      docker.starts.push(id(container));

      if (docker.failStart) {
        throw new Error('start failed');
      }
    },

    async removeContainer(
      _container: ContainerRef | string,
      options: RemoveContainerOptions = {},
    ) {
      docker.removes.push(options);

      if (docker.failRemove) {
        throw new Error('remove failed');
      }
    },

    async exec(_container: ContainerRef | string, input: ExecInput) {
      docker.execs.push(input);

      if (input.cmd.join(' ') === 'git -C /workspace/repo rev-parse HEAD') {
        return ok('abc123\n');
      }

      return ok();
    },

    async putArchive(
      _container: ContainerRef | string,
      input: ArchiveWriteInput,
    ) {
      archives.set(input.path, input.archive);
    },

    async getArchive(
      _container: ContainerRef | string,
      input: ArchiveReadInput,
    ) {
      const directory = input.path.replace(/\/[^/]+$/u, '');
      const archive = archives.get(directory);

      assert.ok(archive, `missing archive for ${input.path}`);

      return archive;
    },
  };

  return docker;
};

test('creates an empty disposable container with safety defaults and no clone', async () => {
  const docker = fakeDocker();
  const session = await createSandbox({
    docker,
    image: 'alpine:latest',
    name: 'doric-context-1',
    resources: {
      memoryBytes: 268_435_456,
      nanoCpus: 1_000_000_000,
      pidsLimit: 128,
      user: '1000:1000',
    },
  });

  assert.equal(session.id, 'container-1');
  assert.deepEqual(docker.starts, ['container-1']);
  assert.equal(docker.execs.length, 0);
  assert.deepEqual(docker.creates[0], {
    name: 'doric-context-1',
    image: 'alpine:latest',
    cmd: ['sh', '-lc', 'while :; do sleep 3600; done'],
    workingDir: '/workspace',
    user: '1000:1000',
    labels: {
      'doric.sandbox': 'true',
      'doric.sandbox.root': '/workspace',
    },
    hostConfig: {
      AutoRemove: false,
      Binds: [],
      NetworkMode: 'none',
      Memory: 268_435_456,
      NanoCpus: 1_000_000_000,
      PidsLimit: 128,
    },
    networkDisabled: true,
  });

  await session.dispose();

  assert.deepEqual(docker.removes, [{ force: true, volumes: true }]);
});

test('rejects invalid custom container names before Docker creation', async () => {
  const docker = fakeDocker();

  await assert.rejects(
    createSandbox({ docker, image: 'alpine:latest', name: 'doric:context' }),
    /Docker container name must match/u,
  );
  assert.equal(docker.creates.length, 0);
});

test('force removes the container when startup fails', async () => {
  const docker = fakeDocker();

  docker.failStart = true;

  await assert.rejects(
    createSandbox({ docker, image: 'alpine:latest' }),
    /start failed/u,
  );
  assert.deepEqual(docker.removes, [
    { force: true, volumes: true, timeoutMs: undefined },
  ]);
});

test('clones repositories only when cloneRepo is called and limits credentials to clone and fetch commands', async () => {
  const docker = fakeDocker();
  const session = await createSandbox({ docker, image: 'alpine:latest' });

  assert.equal(docker.execs.length, 0);

  const repo = await session.cloneRepo({
    url: 'https://example.test/repo.git',
    branch: 'main',
    commit: 'abc123',
    auth: { kind: 'token', token: 'secret-token' },
  });
  const commands = docker.execs.map((input) => input.cmd.join(' '));

  assert.deepEqual(repo, { path: '/workspace/repo', commit: 'abc123' });
  assert.match(commands[0] ?? '', /git clone/u);
  assert.match(commands[0] ?? '', /--branch main/u);
  assert.match(commands[1] ?? '', /git -C \/workspace\/repo fetch/u);
  assert.match(commands[2] ?? '', /git -C \/workspace\/repo checkout/u);
  assert.match(commands[3] ?? '', /git -C \/workspace\/repo rev-parse HEAD/u);

  const encodedCredential = Buffer.from('x-access-token:secret-token').toString(
    'base64',
  );
  assert.ok(commands.every((command) => !command.includes(encodedCredential)));

  const credentialExecs = docker.execs.filter((input) =>
    input.env?.some((value) => value.includes(encodedCredential)),
  );

  assert.deepEqual(
    credentialExecs.map((input) =>
      input.cmd.includes('clone') ? 'clone' : 'fetch',
    ),
    ['clone', 'fetch'],
  );
});

test('writes and reads files through Docker archive APIs', async () => {
  const docker = fakeDocker();
  const session = await createSandbox({ docker, image: 'alpine:latest' });

  await session.writeFile('src/hello.txt', 'hello');

  assert.deepEqual(docker.execs[0]?.cmd, ['mkdir', '-p', '/workspace/src']);
  assert.ok(docker.archives.has('/workspace/src'));

  const content = await session.readFile('/workspace/src/hello.txt');

  assert.equal(content, 'hello');
});

test('rejects file and clone paths outside the sandbox root', async () => {
  const docker = fakeDocker();
  const session = await createSandbox({ docker, image: 'alpine:latest' });

  await assert.rejects(
    session.writeFile('/etc/passwd', 'blocked'),
    /must stay under \/workspace/u,
  );
  await assert.rejects(
    session.cloneRepo({
      url: 'https://example.test/repo.git',
      directory: '/outside/repo',
    }),
    /must stay under \/workspace/u,
  );
});

test('diff runs in the cloned repo by default and accepts an explicit cwd', async () => {
  const docker = fakeDocker();
  const session = await createSandbox({ docker, image: 'alpine:latest' });

  await session.cloneRepo({ url: 'https://example.test/repo.git' });
  const defaultDiff = await session.diff();
  const explicitDiff = await session.diff({ cwd: '/workspace/other' });

  assert.equal(defaultDiff, '');
  assert.equal(explicitDiff, '');

  assert.deepEqual(docker.execs.at(-2), {
    cmd: ['git', 'diff'],
    env: undefined,
    workingDir: '/workspace/repo',
    user: undefined,
    timeoutMs: undefined,
    signal: undefined,
    tty: undefined,
  });
  assert.deepEqual(docker.execs.at(-1), {
    cmd: ['git', 'diff'],
    env: undefined,
    workingDir: '/workspace/other',
    user: undefined,
    timeoutMs: undefined,
    signal: undefined,
    tty: undefined,
  });
});

test('allows dispose to be retried when Docker removal fails', async () => {
  const docker = fakeDocker();
  const session = await createSandbox({ docker, image: 'alpine:latest' });

  docker.failRemove = true;

  await assert.rejects(session.dispose(), /remove failed/u);

  docker.failRemove = false;

  await session.dispose();

  assert.deepEqual(docker.removes, [
    { force: true, volumes: true },
    { force: true, volumes: true },
  ]);
});

const id = (container: ContainerRef | string): string =>
  typeof container === 'string' ? container : container.id;
