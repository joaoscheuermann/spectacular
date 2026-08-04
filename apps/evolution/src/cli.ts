import { Command } from 'commander';

import { initializeWorkspace, type InitSummary } from './init.js';
import type { RunOptions, RunSummary } from './run.js';

type Runner = (options: RunOptions) => Promise<RunSummary>;
type Initializer = (directory?: string) => Promise<InitSummary>;
type Summary = InitSummary | RunSummary;
type Emit = (summary: Summary) => void;

const stdout: Emit = (summary): void => {
  process.stdout.write(JSON.stringify(summary, null, 2) + '\n');
};

/** Creates the Commander surface with injectable execution for CLI tests. */
export const createProgram = (
  run: Runner,
  emit: Emit = stdout,
  init: Initializer = initializeWorkspace,
): Command => {
  const program = new Command()
    .name('evolution')
    .description('Initialize and evolve prompt workspaces.');

  program
    .command('init')
    .description('Initialize a prompt evolution workspace.')
    .argument('[directory]', 'workspace directory', '.')
    .action(async (directory: string) => {
      emit(await init(directory));
    });

  program
    .command('evolve')
    .description('Evolve a prompt independently for each configured model.')
    .argument('<config-path>', 'path to evolution.config.json')
    .option('--dry-run', 'execute evolution without filesystem writes', false)
    .action(
      async (
        config: string,
        options: {
          readonly dryRun: boolean;
        },
      ) => {
        emit(
          await run({
            config,
            dryRun: options.dryRun,
          }),
        );
      },
    );

  return program;
};
