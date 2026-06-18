import {
  resolveDoricListenOptions,
  startDoricServer,
  type StartedDoricServer,
} from './lib/start.js';

const run = async (): Promise<void> => {
  const options = resolveDoricListenOptions();
  const started = await startDoricServer({
    ...options,
    writeLine: (line) => {
      console.log(line);
    },
  });

  installShutdownHandlers(started);
};

const installShutdownHandlers = (started: StartedDoricServer): void => {
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

void run().catch((error: unknown) => {
  console.error(toErrorMessage(error));
  process.exitCode = 1;
});
