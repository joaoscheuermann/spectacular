import type { Server } from 'node:http';

import { createServer as createA2aServer } from './server.js';

export const DEFAULT_HOST = '127.0.0.1';
export const DEFAULT_PORT = 4123;

export type ListenOptions = {
  readonly host: string;
  readonly port: number;
};

export type StartServerOptions = Partial<ListenOptions> & {
  readonly createServer?: () => Server;
  readonly writeLine?: (line: string) => void;
};

export type StartedServer = ListenOptions & {
  readonly origin: string;
  readonly server: Server;
  readonly close: () => Promise<void>;
};

type ServerEnv = Readonly<Record<string, string | undefined>>;

const MAX_PORT = 65_535;
const PORT_PATTERN = /^\d+$/u;

/** Resolves host and port settings for the local Doric A2A server. */
export const resolveListenOptions = (
  env: ServerEnv = process.env,
): ListenOptions => ({
  host: env.HOST ?? DEFAULT_HOST,
  port: parseEnvPort(env.PORT),
});

/** Starts Doric's local A2A HTTP server and returns its resolved address. */
export const startServer = async (
  options: StartServerOptions = {},
): Promise<StartedServer> => {
  const host = options.host ?? DEFAULT_HOST;
  const port = validatePort(options.port ?? DEFAULT_PORT, 'port');
  const server = (options.createServer ?? createA2aServer)();

  await listen(server, host, port);

  const actualPort = getListeningPort(server, port);
  const origin = `http://${host}:${actualPort}`;
  const started = {
    server,
    host,
    port: actualPort,
    origin,
    close: () => close(server),
  };

  try {
    options.writeLine?.(formatListenMessage(origin));
  } catch (error) {
    await started.close();
    throw error;
  }

  return started;
};

export const formatListenMessage = (origin: string): string =>
  `Doric A2A agent listening on ${origin}`;

const parseEnvPort = (value: string | undefined): number => {
  if (value === undefined) {
    return DEFAULT_PORT;
  }

  if (!PORT_PATTERN.test(value)) {
    throw new Error(formatInvalidPortMessage(value, 'PORT'));
  }

  return validatePort(Number(value), 'PORT', value);
};

const validatePort = (
  value: number,
  source: 'PORT' | 'port',
  rawValue = String(value),
): number => {
  if (!Number.isInteger(value) || value < 0 || value > MAX_PORT) {
    throw new Error(formatInvalidPortMessage(rawValue, source));
  }

  return value;
};

const formatInvalidPortMessage = (
  value: string,
  source: 'PORT' | 'port',
): string =>
  `${source} must be an integer from 0 to ${MAX_PORT}; received ${JSON.stringify(value)}.`;

const listen = (server: Server, host: string, port: number): Promise<void> =>
  new Promise((resolve, reject) => {
    const cleanup = (): void => {
      server.off('error', onError);
      server.off('listening', onListening);
    };
    const onError = (error: Error): void => {
      cleanup();
      reject(error);
    };
    const onListening = (): void => {
      cleanup();
      resolve();
    };

    server.once('error', onError);
    server.once('listening', onListening);
    server.listen(port, host);
  });

const getListeningPort = (server: Server, fallback: number): number => {
  const address = server.address();

  if (address === null || typeof address === 'string') {
    return fallback;
  }

  return address.port;
};

const close = (server: Server): Promise<void> =>
  new Promise((resolve, reject) => {
    server.close((error) => {
      if (error) {
        reject(error);
        return;
      }

      resolve();
    });
  });
