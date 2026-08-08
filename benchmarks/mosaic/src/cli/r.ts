import { access, readFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';

import { progress } from './io.js';

const exists = async (path: string): Promise<boolean> =>
  access(path).then(
    () => true,
    () => false,
  );

const packageRoot = async (): Promise<string> => {
  const current = dirname(fileURLToPath(import.meta.url));
  const candidates = [
    resolve(process.cwd(), 'benchmarks/mosaic'),
    resolve(current, '../..'),
    resolve(current, '../../..'),
  ];
  for (const candidate of candidates) {
    if (await exists(join(candidate, 'analysis', 'analyze.R')))
      return candidate;
  }
  throw new Error('benchmark analysis directory is unavailable');
};

const runProcess = async (
  program: string,
  args: readonly string[],
): Promise<string> =>
  new Promise((resolvePromise, reject) => {
    let stdout = '';
    const child = spawn(program, [...args], {
      cwd: process.cwd(),
      env: process.env,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    child.stdout.on('data', (chunk: Buffer) => {
      const text = chunk.toString('utf8');
      stdout += text;
      const message = text.trimEnd();
      if (message.length > 0) progress(message);
    });
    child.stderr.on('data', (chunk: Buffer) =>
      progress(chunk.toString('utf8').trimEnd()),
    );
    child.once('error', () => reject(new Error(`${program} is unavailable`)));
    child.once('close', (code) => {
      if (code === 0) resolvePromise(stdout);
      else reject(new Error(`${program} exited unsuccessfully`));
    });
  });

export const runContainerVerifier = async (
  script: 'verify-power.sh' | 'verify-determinism.sh',
  args: readonly string[],
): Promise<string> => {
  const root = await packageRoot();
  const output = await runProcess(join(root, 'container', script), args);
  const directory = output.trim().split(/\r?\n/u).at(-1);
  if (directory === undefined || directory.length === 0) {
    throw new Error('container verifier returned no artifact directory');
  }
  return directory;
};

export const readAnalysisResult = async (path: string): Promise<unknown> =>
  JSON.parse(await readFile(path, 'utf8')) as unknown;
