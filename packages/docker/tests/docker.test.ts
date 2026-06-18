import { Buffer } from 'node:buffer';
import assert from 'node:assert/strict';
import test from 'node:test';

import {
  DockerHttpError,
  DockerRequestAbortedError,
  DockerRequestTimeoutError,
  createDockerClient,
  type DockerResponse,
  type DockerTransport,
  type DockerTransportRequest,
} from '../src/index.js';

const jsonResponse = (status: number, body: unknown): DockerResponse => ({
  status,
  headers: {},
  body: Buffer.from(JSON.stringify(body)),
});

const textResponse = (status: number, body: string): DockerResponse => ({
  status,
  headers: {},
  body: Buffer.from(body),
});

test('sends create container requests with Docker API paths and JSON body', async () => {
  const requests: DockerTransportRequest[] = [];
  const client = createDockerClient({
    request: async (request) => {
      requests.push(request);
      return jsonResponse(201, { Id: 'container-1', Warnings: ['careful'] });
    },
  });

  const container = await client.createContainer({
    image: 'alpine:latest',
    name: 'workspace',
    cmd: ['sleep', '60'],
    env: ['A=B'],
    workingDir: '/workspace',
    labels: { owner: 'doric' },
    hostConfig: { NetworkMode: 'none' },
    networkDisabled: true,
  });

  assert.deepEqual(container, { id: 'container-1', warnings: ['careful'] });
  assert.equal(requests[0]?.method, 'POST');
  assert.equal(requests[0]?.path, '/containers/create');
  assert.deepEqual(requests[0]?.query, { name: 'workspace' });
  assert.deepEqual(requests[0]?.body, {
    Image: 'alpine:latest',
    Cmd: ['sleep', '60'],
    Env: ['A=B'],
    WorkingDir: '/workspace',
    Labels: { owner: 'doric' },
    HostConfig: { NetworkMode: 'none' },
    NetworkDisabled: true,
  });
});

test('maps Docker HTTP failures into structured errors', async () => {
  const client = createDockerClient({
    request: async () => textResponse(500, 'daemon failed'),
  });

  await assert.rejects(
    client.version(),
    (error: unknown) =>
      error instanceof DockerHttpError &&
      error.status === 500 &&
      error.method === 'GET' &&
      error.path === '/version' &&
      error.body === 'daemon failed',
  );
});

test('honors timeout and abort controls around the injected transport', async () => {
  const never: DockerTransport = async () => new Promise(() => undefined);
  const timeoutClient = createDockerClient({
    request: never,
    timeoutMs: 1,
  });

  await assert.rejects(
    timeoutClient.ping(),
    (error: unknown) => error instanceof DockerRequestTimeoutError,
  );

  const abortClient = createDockerClient({ request: never });
  const controller = new AbortController();
  const pending = abortClient.ping({ signal: controller.signal });

  controller.abort();

  await assert.rejects(
    pending,
    (error: unknown) => error instanceof DockerRequestAbortedError,
  );
});

test('creates starts and inspects execs while demuxing non TTY output', async () => {
  const requests: DockerTransportRequest[] = [];
  const client = createDockerClient({
    request: async (request) => {
      requests.push(request);

      if (request.path === '/containers/container-1/exec') {
        return jsonResponse(201, { Id: 'exec-1' });
      }

      if (request.path === '/exec/exec-1/start') {
        return {
          status: 200,
          headers: {},
          body: Buffer.concat([frame(1, 'out\n'), frame(2, 'err\n')]),
        };
      }

      return jsonResponse(200, { ExitCode: 7 });
    },
  });

  const result = await client.exec('container-1', {
    cmd: ['sh', '-lc', 'echo out'],
    workingDir: '/workspace',
    env: ['A=B'],
  });

  assert.deepEqual(
    requests.map((request) => [request.method, request.path]),
    [
      ['POST', '/containers/container-1/exec'],
      ['POST', '/exec/exec-1/start'],
      ['GET', '/exec/exec-1/json'],
    ],
  );
  assert.deepEqual(requests[0]?.body, {
    AttachStdout: true,
    AttachStderr: true,
    Tty: false,
    Cmd: ['sh', '-lc', 'echo out'],
    Env: ['A=B'],
    WorkingDir: '/workspace',
  });
  assert.equal(result.exitCode, 7);
  assert.equal(result.stdout, 'out\n');
  assert.equal(result.stderr, 'err\n');
});

test('uses Docker archive endpoints for upload and download', async () => {
  const requests: DockerTransportRequest[] = [];
  const client = createDockerClient({
    request: async (request) => {
      requests.push(request);

      if (request.method === 'GET') {
        return textResponse(200, 'archive');
      }

      return textResponse(200, '');
    },
  });

  await client.putArchive('container-1', {
    path: '/workspace',
    archive: Buffer.from('tar'),
    copyUidGid: true,
  });
  const archive = await client.getArchive('container-1', {
    path: '/workspace/file.txt',
  });

  assert.equal(requests[0]?.method, 'PUT');
  assert.equal(requests[0]?.path, '/containers/container-1/archive');
  assert.deepEqual(requests[0]?.query, {
    path: '/workspace',
    noOverwriteDirNonDir: undefined,
    copyUIDGID: true,
  });
  assert.equal(Buffer.from(archive).toString('utf8'), 'archive');
});

const frame = (channel: number, text: string): Buffer => {
  const body = Buffer.from(text);
  const header = Buffer.alloc(8);

  header[0] = channel;
  header.writeUInt32BE(body.byteLength, 4);

  return Buffer.concat([header, body]);
};
