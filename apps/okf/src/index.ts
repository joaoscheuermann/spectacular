import fs from 'node:fs';
import path from 'node:path';

import { Command } from 'commander';

import pino from 'pino';
import pretty from 'pino-pretty';

import { readGitIgnoreFile } from './lib/git.js';
import { walk } from './lib/walk.js';
import { classify } from './lib/agents/classify/index.js';

const logger = pino(pretty());

/** Runs the OKF CLI against the repository path passed through --path. */
async function main(): Promise<void> {
  // Logger

  const program = new Command()
    .name('okf')
    .description('Generate an Open Knowledge Format bundle for a repository.')
    .requiredOption('--path <repo-root>', 'repository root to analyze');

  program.parse();

  const options = program.opts<{
    readonly path: string;
  }>();

  logger.info(
    {
      path: options.path,
    },
    'okf.start',
  );

  const root = options.path;
  const output = path.join(options.path, '.doric', 'knowledge');

  // Ensure the root folder is created
  try {
    await fs.promises.access(output, fs.constants.F_OK);
    logger.info(
      {
        path: output,
      },
      'okf:output folder already exists',
    );
  } catch {
    await fs.promises.mkdir(output, { recursive: true });
    logger.info(
      {
        path: output,
      },
      'okf:output folder created',
    );
  }

  // Root .gitignore
  const ignore = await readGitIgnoreFile(root);

  // We walk file by file
  await walk(
    root,
    async (root, target) => {
      const stat = await fs.promises.stat(target);

      if (stat.isDirectory()) {
        console.log('Go depper');
      } else {
        const filePath = path.join(root, target);

        const body = await fs.promises.readFile(filePath, 'utf-8');

        const classification = await classify(filePath, body);

        console.log('OUTPUT', classification.structured);
      }
    },
    {
      ignore,
    },
  );
}

main()
  .then(() => {
    logger.info('Finished!');
  })
  .catch((error) => logger.error(error.message));
