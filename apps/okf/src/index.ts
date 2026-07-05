#!/usr/bin/env node
import { Command } from 'commander';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import pino from 'pino';

import { generateKnowledgeBundle } from './lib/analyzer.js';
import { createLmStudioSummarizer } from './lib/summarizer.js';
import type { KnowledgeLogger } from './lib/types.js';

export * from './lib/analyzer.js';
export * from './lib/collect.js';
export * from './lib/extract.js';
export * from './lib/log.js';
export * from './lib/markdown.js';
export * from './lib/summarizer.js';
export * from './lib/tree.js';
export * from './lib/types.js';

export type CliOptions = {
  readonly path: string;
};

export const createCliLogger = (): KnowledgeLogger =>
  pino(
    {
      base: undefined,
      level: process.env['OKF_LOG_LEVEL'] ?? 'info',
      timestamp: false,
    },
    pino.destination(2),
  );

/** Runs the OKF CLI against the repository path passed through --path. */
export const runCli = async (argv: readonly string[]): Promise<void> => {
  const logger = createCliLogger();
  const program = new Command()
    .name('okf')
    .description('Generate an Open Knowledge Format bundle for a repository.')
    .requiredOption('--path <repo-root>', 'repository root to analyze');

  program.parse(argv, { from: 'user' });
  const options = program.opts<CliOptions>();
  const result = await generateKnowledgeBundle({
    rootPath: options.path,
    summarizer: createLmStudioSummarizer(),
    logger,
  });

  logger.info(
    {
      knowledgePath: result.knowledgePath,
      files: result.filesWritten,
      indexes: result.indexesWritten,
      logs: result.logsWritten,
    },
    'okf.result',
  );
};

const isEntrypoint = (): boolean =>
  process.argv[1] !== undefined &&
  fileURLToPath(import.meta.url) === path.resolve(process.argv[1]);

if (isEntrypoint()) {
  runCli(process.argv.slice(2)).catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    console.error(message);
    process.exitCode = 1;
  });
}
