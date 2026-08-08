#!/usr/bin/env node

import { executeCommand } from './cli/commands.js';
import { parseInvocation } from './cli/args.js';
import { jsonLine, progress } from './cli/io.js';

const safeMessage = (error: unknown): string =>
  error instanceof Error && error.message.trim().length > 0
    ? error.message
    : 'benchmark command failed';

const main = async (): Promise<void> => {
  try {
    const invocation = parseInvocation(process.argv.slice(2));
    progress(`mosaic-benchmark: ${invocation.command} started`);
    const output = await executeCommand(invocation);
    process.stdout.write(
      jsonLine({
        schemaVersion: 1,
        ok: output.exitCode === undefined,
        command: invocation.command,
        result: output.result,
      }),
    );
    process.exitCode = output.exitCode ?? 0;
    progress(`mosaic-benchmark: ${invocation.command} finished`);
  } catch (error) {
    process.stdout.write(
      jsonLine({
        schemaVersion: 1,
        ok: false,
        error: { code: 'command_failed', message: safeMessage(error) },
      }),
    );
    process.exitCode = 1;
    progress('mosaic-benchmark: command failed');
  }
};

await main();
