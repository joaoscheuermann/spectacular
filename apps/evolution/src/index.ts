#!/usr/bin/env node
import pino from 'pino';
import pretty from 'pino-pretty';

import { createProgram } from './cli.js';
import type { Progress } from './progress.js';
import { runEvolution } from './run.js';

const logger = pino(
  {
    redact: {
      paths: [
        'token',
        '*.token',
        'apiKey',
        '*.apiKey',
        'authorization',
        '*.authorization',
        'headers.authorization',
        '*.headers.authorization',
      ],
      censor: '[Redacted]',
    },
  },
  pretty({ destination: 2, colorize: process.stderr.isTTY }),
);

const progress: Progress = ({ event, ...fields }) => logger.info(fields, event);

try {
  await createProgram((options) =>
    runEvolution(options, { progress }),
  ).parseAsync(process.argv);
} catch (error) {
  logger.error(
    error instanceof Error
      ? { errorName: error.name, message: error.message }
      : { message: 'Unknown error.' },
    'command.failed',
  );
  process.exitCode = 1;
}
