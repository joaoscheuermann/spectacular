import { Buffer } from 'node:buffer';
import {
  request as httpRequest,
  type IncomingHttpHeaders,
  type RequestOptions,
} from 'node:http';
import { TextDecoder } from 'node:util';

import {
  DockerHttpError,
  DockerRequestAbortedError,
  DockerRequestTimeoutError,
} from './classes/errors.js';
import type {
  ContainerRef,
  CreateContainerInput,
  DockerClient,
  DockerConnection,
  DockerResponse,
  DockerTransport,
  DockerTransportRequest,
  DockerVersion,
  ExecInput,
} from './types/docker.js';
import { demuxDockerOutput } from './utils/demux.js';

export type CreateDockerClientOptions = {
  readonly connection?: DockerConnection;
  readonly request?: DockerTransport;
  readonly timeoutMs?: number;
};

const decoder = new TextDecoder();

/** Creates a Docker Engine API client over Unix sockets or Windows named pipes. */
export const createDockerClient = (
  options: CreateDockerClientOptions = {},
): DockerClient => {
  const request =
    options.request ??
    createNodeTransport(options.connection ?? defaultConnection());
  const timeoutMs = options.timeoutMs;
  const send = (
    input: DockerTransportRequest,
    expectedStatus: number | readonly number[],
  ) =>
    sendDockerRequest(
      request,
      withDefaultTimeout(input, timeoutMs),
      expectedStatus,
    );

  const json = async <Value>(
    input: DockerTransportRequest,
    expectedStatus: number | readonly number[],
  ): Promise<Value> => parseJson((await send(input, expectedStatus)).body);

  return {
    async ping(control = {}) {
      await send({ method: 'GET', path: '/_ping', ...control }, 200);
    },

    async version(control = {}) {
      const raw = await json<Record<string, unknown>>(
        { method: 'GET', path: '/version', ...control },
        200,
      );

      return versionFrom(raw);
    },

    async pullImage(input, control = {}) {
      await send(
        {
          method: 'POST',
          path: '/images/create',
          query: { fromImage: input.image },
          ...control,
        },
        200,
      );
    },

    async createContainer(input, control = {}) {
      const response = await json<Record<string, unknown>>(
        {
          method: 'POST',
          path: '/containers/create',
          query: { name: input.name },
          body: createBody(input),
          ...control,
        },
        201,
      );

      return containerFrom(response);
    },

    async startContainer(container, control = {}) {
      await send(
        {
          method: 'POST',
          path: `/containers/${encodeURIComponent(containerId(container))}/start`,
          ...control,
        },
        [204, 304],
      );
    },

    async removeContainer(container, options = {}) {
      await send(
        {
          method: 'DELETE',
          path: `/containers/${encodeURIComponent(containerId(container))}`,
          query: {
            force: options.force,
            v: options.volumes,
            link: options.link,
          },
          signal: options.signal,
          timeoutMs: options.timeoutMs,
        },
        [204, 404],
      );
    },

    async exec(container, input) {
      const created = await json<Record<string, unknown>>(
        {
          method: 'POST',
          path: `/containers/${encodeURIComponent(containerId(container))}/exec`,
          body: execCreateBody(input),
          signal: input.signal,
          timeoutMs: input.timeoutMs,
        },
        201,
      );
      const execId = stringField(created, 'Id');
      const stream = await send(
        {
          method: 'POST',
          path: `/exec/${encodeURIComponent(execId)}/start`,
          body: { Detach: false, Tty: input.tty ?? false },
          signal: input.signal,
          timeoutMs: input.timeoutMs,
        },
        200,
      );
      const inspected = await json<Record<string, unknown>>(
        {
          method: 'GET',
          path: `/exec/${encodeURIComponent(execId)}/json`,
          signal: input.signal,
          timeoutMs: input.timeoutMs,
        },
        200,
      );
      const output =
        input.tty === true
          ? { stdout: stream.body, stderr: new Uint8Array() }
          : demuxDockerOutput(stream.body);

      return {
        exitCode: numberField(inspected, 'ExitCode'),
        stdout: decoder.decode(output.stdout),
        stderr: decoder.decode(output.stderr),
        stdoutBytes: output.stdout,
        stderrBytes: output.stderr,
      };
    },

    async putArchive(container, input, control = {}) {
      await send(
        {
          method: 'PUT',
          path: `/containers/${encodeURIComponent(containerId(container))}/archive`,
          query: {
            path: input.path,
            noOverwriteDirNonDir: input.noOverwriteDirNonDir,
            copyUIDGID: input.copyUidGid,
          },
          body: input.archive,
          ...control,
        },
        200,
      );
    },

    async getArchive(container, input, control = {}) {
      return (
        await send(
          {
            method: 'GET',
            path: `/containers/${encodeURIComponent(containerId(container))}/archive`,
            query: { path: input.path },
            ...control,
          },
          200,
        )
      ).body;
    },
  };
};

const createNodeTransport =
  (connection: DockerConnection): DockerTransport =>
  (request) =>
    new Promise<DockerResponse>((resolve, reject) => {
      const body = encodeBody(request.body);
      const options: RequestOptions = {
        method: request.method,
        path: pathWithQuery(request),
        socketPath: socketPath(connection),
        headers: { ...request.headers, ...body.headers },
      };
      const client = httpRequest(options, (response) => {
        const chunks: Uint8Array[] = [];

        response.on('data', (chunk: Uint8Array) => chunks.push(chunk));
        response.on('end', () => {
          cleanup();
          resolve({
            status: response.statusCode ?? 0,
            headers: headersFrom(response.headers),
            body: Buffer.concat(chunks),
          });
        });
      });
      const abort = () => {
        client.destroy(
          new DockerRequestAbortedError(request.method, request.path),
        );
      };
      const cleanup = () => {
        request.signal?.removeEventListener('abort', abort);
      };

      client.on('error', (error) => {
        cleanup();
        reject(error);
      });

      if (request.signal?.aborted === true) {
        abort();
        return;
      }

      request.signal?.addEventListener('abort', abort, { once: true });

      if (body.bytes !== undefined) {
        client.write(body.bytes);
      }

      client.end();
    });

const sendDockerRequest = async (
  transport: DockerTransport,
  request: DockerTransportRequest,
  expectedStatus: number | readonly number[],
): Promise<DockerResponse> => {
  const response = await withRequestControl(transport, request);
  const expected = Array.isArray(expectedStatus)
    ? expectedStatus
    : [expectedStatus];

  if (!expected.includes(response.status)) {
    throw new DockerHttpError({
      status: response.status,
      method: request.method,
      path: pathWithQuery(request),
      body: decoder.decode(response.body),
    });
  }

  return response;
};

const withRequestControl = async (
  transport: DockerTransport,
  request: DockerTransportRequest,
): Promise<DockerResponse> => {
  if (request.signal?.aborted === true) {
    throw new DockerRequestAbortedError(request.method, request.path);
  }

  const controller = new AbortController();
  let timedOut = false;
  const abortFromInput = () => controller.abort();
  const timeout = request.timeoutMs;
  const timer =
    timeout === undefined
      ? undefined
      : setTimeout(() => {
          timedOut = true;
          controller.abort();
        }, timeout);
  const abortPromise = new Promise<never>((_resolve, reject) => {
    const onAbort = () => {
      reject(
        timedOut
          ? new DockerRequestTimeoutError(
              request.method,
              request.path,
              timeout ?? 0,
            )
          : new DockerRequestAbortedError(request.method, request.path),
      );
    };

    controller.signal.addEventListener('abort', onAbort, { once: true });
  });

  request.signal?.addEventListener('abort', abortFromInput, { once: true });

  try {
    return await Promise.race([
      transport({ ...request, signal: controller.signal }),
      abortPromise,
    ]);
  } finally {
    request.signal?.removeEventListener('abort', abortFromInput);

    if (timer !== undefined) {
      clearTimeout(timer);
    }
  }
};

const createBody = (input: CreateContainerInput): Record<string, unknown> => ({
  Image: input.image,
  ...(input.cmd === undefined ? {} : { Cmd: [...input.cmd] }),
  ...(input.env === undefined ? {} : { Env: [...input.env] }),
  ...(input.workingDir === undefined ? {} : { WorkingDir: input.workingDir }),
  ...(input.labels === undefined ? {} : { Labels: { ...input.labels } }),
  ...(input.user === undefined ? {} : { User: input.user }),
  ...(input.hostConfig === undefined ? {} : { HostConfig: input.hostConfig }),
  ...(input.networkDisabled === undefined
    ? {}
    : { NetworkDisabled: input.networkDisabled }),
});

const execCreateBody = (input: ExecInput): Record<string, unknown> => ({
  AttachStdout: true,
  AttachStderr: true,
  Tty: input.tty ?? false,
  Cmd: [...input.cmd],
  ...(input.env === undefined ? {} : { Env: [...input.env] }),
  ...(input.workingDir === undefined ? {} : { WorkingDir: input.workingDir }),
  ...(input.user === undefined ? {} : { User: input.user }),
});

const encodeBody = (
  body: unknown,
): {
  readonly bytes?: Uint8Array;
  readonly headers: Readonly<Record<string, string>>;
} => {
  if (body === undefined) {
    return { headers: {} };
  }

  if (body instanceof Uint8Array) {
    return {
      bytes: body,
      headers: { 'content-length': String(body.byteLength) },
    };
  }

  const bytes = Buffer.from(JSON.stringify(body));

  return {
    bytes,
    headers: {
      'content-type': 'application/json',
      'content-length': String(bytes.byteLength),
    },
  };
};

const parseJson = <Value>(body: Uint8Array): Value => {
  if (body.byteLength === 0) {
    return {} as Value;
  }

  return JSON.parse(decoder.decode(body)) as Value;
};

const versionFrom = (raw: Record<string, unknown>): DockerVersion => ({
  version: stringValue(raw.Version),
  apiVersion: stringValue(raw.ApiVersion),
  minApiVersion: stringValue(raw.MinAPIVersion),
  gitCommit: stringValue(raw.GitCommit),
  goVersion: stringValue(raw.GoVersion),
  os: stringValue(raw.Os),
  arch: stringValue(raw.Arch),
  kernelVersion: stringValue(raw.KernelVersion),
  experimental:
    typeof raw.Experimental === 'boolean' ? raw.Experimental : undefined,
  raw,
});

const containerFrom = (response: Record<string, unknown>): ContainerRef => ({
  id: stringField(response, 'Id'),
  warnings: Array.isArray(response.Warnings)
    ? response.Warnings.filter(
        (warning): warning is string => typeof warning === 'string',
      )
    : [],
});

const stringField = (record: Record<string, unknown>, key: string): string => {
  const value = record[key];

  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`Docker response is missing string field: ${key}`);
  }

  return value;
};

const numberField = (
  record: Record<string, unknown>,
  key: string,
): number | null => {
  const value = record[key];

  return typeof value === 'number' ? value : null;
};

const stringValue = (value: unknown): string | undefined =>
  typeof value === 'string' ? value : undefined;

const containerId = (container: ContainerRef | string): string =>
  typeof container === 'string' ? container : container.id;

const withDefaultTimeout = (
  request: DockerTransportRequest,
  timeoutMs: number | undefined,
): DockerTransportRequest => ({
  ...request,
  timeoutMs: request.timeoutMs ?? timeoutMs,
});

const defaultConnection = (): DockerConnection =>
  process.platform === 'win32' ? { kind: 'namedPipe' } : { kind: 'unix' };

const socketPath = (connection: DockerConnection): string =>
  connection.kind === 'unix'
    ? (connection.socketPath ??
      dockerHostSocketPath(process.env.DOCKER_HOST) ??
      '/var/run/docker.sock')
    : (connection.pipePath ?? '//./pipe/docker_engine');

const dockerHostSocketPath = (
  dockerHost: string | undefined,
): string | undefined => {
  if (dockerHost === undefined || dockerHost.length === 0) {
    return undefined;
  }

  if (dockerHost.startsWith('unix://')) {
    return dockerHost.slice('unix://'.length);
  }

  return dockerHost.startsWith('/') ? dockerHost : undefined;
};

const pathWithQuery = (request: DockerTransportRequest): string => {
  const params = new URLSearchParams();

  for (const [key, value] of Object.entries(request.query ?? {})) {
    if (value !== undefined) {
      params.set(key, String(value));
    }
  }

  const query = params.toString();

  return query.length === 0 ? request.path : `${request.path}?${query}`;
};

const headersFrom = (
  headers: IncomingHttpHeaders,
): Record<string, string | readonly string[] | undefined> => ({ ...headers });
