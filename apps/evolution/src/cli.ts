import { Command } from 'commander';

import { runEvolution, type RunOptions, type RunSummary } from './run.js';

type Runner = (options: RunOptions) => Promise<RunSummary>;
type Emit = (summary: RunSummary) => void;

const stdout: Emit = (summary) => {
  process.stdout.write(JSON.stringify(summary, null, 2) + '\n');
};

/** Creates the Commander surface with injectable execution for CLI tests. */
export const createProgram = (
  run: Runner = runEvolution,
  emit: Emit = stdout,
): Command =>
  new Command()
    .name('evolve')
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
