import { spawn } from 'node:child_process';

import type { CommandRunner } from './campaign-types.js';

type Output = Pick<NodeJS.WriteStream, 'write'>;

/** Runs benchmark commands while mirroring their output live to stderr. */
export const createProcessRunner =
  (
    environment: NodeJS.ProcessEnv,
    output: Output = process.stderr,
  ): CommandRunner =>
  ({ file, args, cwd, env }) =>
    new Promise((resolveResult, reject) => {
      const child = spawn(file, args as string[], {
        cwd,
        env: { ...environment, ...env },
        stdio: ['ignore', 'pipe', 'pipe'],
      });
      let stdout = '';
      let stderr = '';

      child.stdout.on('data', (chunk: Buffer) => {
        const value = chunk.toString();

        stdout += value;

        output.write(value);
      });

      child.stderr.on('data', (chunk: Buffer) => {
        const value = chunk.toString();

        stderr += value;

        output.write(value);
      });

      child.once('error', reject);

      child.once('close', (code) =>
        resolveResult({ code: code ?? 1, stdout, stderr }),
      );
    });
