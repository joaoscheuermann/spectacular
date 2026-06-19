import { pathToFileURL } from 'node:url';

import {
  resolveListenOptions,
  startServer,
  type StartedServer,
} from './lib/start.js';

export * from './lib/card.js';
export * from './lib/executor.js';
export * from './lib/messages/hello-world.js';
export * from './lib/server.js';
export * from './lib/start.js';

export const run = async (): Promise<void> => {
  const options = resolveListenOptions();
  const started = await startServer({
    ...options,
    writeLine: (line) => {
      console.log(line);
    },
  });

  installShutdownHandlers(started);
};

const installShutdownHandlers = (started: StartedServer): void => {
  const shutdown = (): void => {
    process.off('SIGINT', shutdown);
    process.off('SIGTERM', shutdown);

    void started.close().catch((error: unknown) => {
      console.error(toErrorMessage(error));
      process.exitCode = 1;
    });
  };

  process.once('SIGINT', shutdown);
  process.once('SIGTERM', shutdown);
};

const toErrorMessage = (error: unknown): string => {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
};

const isDirectExecution = (): boolean =>
  process.argv[1] !== undefined &&
  import.meta.url === pathToFileURL(process.argv[1]).href;

if (isDirectExecution()) {
  void run().catch((error: unknown) => {
    console.error(toErrorMessage(error));
    process.exitCode = 1;
  });
}
