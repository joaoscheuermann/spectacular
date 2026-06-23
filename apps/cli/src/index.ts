#!/usr/bin/env node
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import { Command } from 'commander';
import inquirer from 'inquirer';

import {
  runConnect,
  runFeature,
  runKill,
  runList,
  type CommandDependencies,
} from './lib/commands.js';
import type { Prompt } from './lib/questions.js';

const createProgram = (dependencies: CommandDependencies): Command => {
  const program = new Command();

  program
    .name('doric')
    .description('Doric A2A command-line client.')
    .exitOverride()
    .configureOutput({
      writeOut: (text) => dependencies.io.stdout.write(text),
      writeErr: (text) => dependencies.io.stderr.write(text),
    });

  program
    .command('feature')
    .requiredOption('--prompt <text>', 'Prompt to send to Doric.')
    .requiredOption('--repo <url>', 'GitHub repository URL to clone.')
    .option('--branch <name>', 'Git branch to clone.')
    .action((options: { prompt: string; repo: string; branch?: string }) =>
      runFeature(options, dependencies),
    );

  program.command('list').action(() => runList(dependencies));

  program
    .command('connect')
    .argument('<contextId>')
    .action((contextId: string) => runConnect(contextId, dependencies));

  program
    .command('kill')
    .argument('<contextId>')
    .action((contextId: string) => runKill(contextId, dependencies));

  return program;
};

export const run = async (
  argv = process.argv,
  dependencies: CommandDependencies = {
    io: processIo,
    fetch,
    prompt: inquirer.prompt as Prompt,
  },
): Promise<void> => {
  try {
    await createProgram(dependencies).parseAsync(argv);
  } catch (error) {
    if (isHelpDisplayed(error)) {
      return;
    }

    throw error;
  }
};

const processIo = {
  stdout: { write: (text: string) => process.stdout.write(text) },
  stderr: { write: (text: string) => process.stderr.write(text) },
};

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

const isHelpDisplayed = (error: unknown): boolean =>
  isRecord(error) &&
  error['code'] === 'commander.helpDisplayed' &&
  error['exitCode'] === 0;

const isDirectRun = (moduleUrl: string, argvPath: string | undefined): boolean =>
  argvPath !== undefined && moduleUrl === pathToFileURL(resolve(argvPath)).href;

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;

if (isDirectRun(import.meta.url, process.argv[1])) {
  await run().catch((error: unknown) => {
    process.stderr.write(`${errorMessage(error)}\n`);
    process.exitCode = 1;
  });
}
