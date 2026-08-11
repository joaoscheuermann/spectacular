import { stat } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { resolve } from 'node:path';
import { ToolDefinitionSchema, type Tool } from 'tool';
import { z } from 'zod';

const DEFAULT_TIMEOUT_MS = 120_000;
const MAX_TIMEOUT_MS = 600_000;
const OUTPUT_LIMIT_BYTES = 12 * 1024;
const KILL_GRACE_MS = 100;
const description =
  'Executes a shell command in the benchmark session and returns compact output.';
const sensitiveName =
  /(?:master|private|api|access)[_-]?key|auth(?:orization)?|bearer|token|secret|password|credentials?|cookie/iu;

const input = z
  .object({
    command: z.string(),
    working_directory: z.string().optional(),
    timeout_ms: z.number().int().positive().max(MAX_TIMEOUT_MS).optional(),
    max_output_chars: z
      .number()
      .int()
      .positive()
      .max(OUTPUT_LIMIT_BYTES)
      .optional(),
  })
  .strict();

const output = z
  .object({
    exit_code: z.number().int(),
    stdout: z.string(),
    stderr: z.string(),
    timed_out: z.boolean(),
    truncated: z.boolean(),
  })
  .strict();

type TerminalInput = z.output<typeof input>;
type TerminalOutput = z.output<typeof output>;
type Capture = {
  readonly chunks: Buffer[];
  size: number;
  truncated: boolean;
};

const definition = ToolDefinitionSchema.parse({
  name: 'terminal',
  description,
  inputSchema: z.toJSONSchema(input, { io: 'output' }),
  outputSchema: z.toJSONSchema(output, { io: 'output' }),
  strict: true,
});

const abortError = (): Error =>
  Object.assign(new Error('The operation was aborted'), { name: 'AbortError' });

const isAborted = (signal: AbortSignal | undefined): boolean =>
  signal?.aborted === true;

const workingDirectory = async (
  cwd: string,
  requested: string | undefined,
): Promise<string> => {
  const directory = resolve(cwd, requested ?? '.');
  let details;

  try {
    details = await stat(directory);
  } catch {
    throw new Error(`Working directory does not exist: ${directory}`);
  }

  if (!details.isDirectory()) {
    throw new Error(`Working directory is not a directory: ${directory}`);
  }

  return directory;
};

const capture = (): Capture => ({ chunks: [], size: 0, truncated: false });

const environment = (): NodeJS.ProcessEnv =>
  Object.fromEntries(
    Object.entries(process.env).filter(([name]) => !sensitiveName.test(name)),
  );

const append = (
  target: Capture,
  chunk: Buffer,
  used: number,
  limit: number,
): void => {
  const available = limit - used;

  if (available <= 0) {
    target.truncated = true;
    return;
  }

  target.chunks.push(chunk.subarray(0, available));
  target.size += Math.min(chunk.length, available);
  target.truncated ||= chunk.length > available;
};

const terminate = (
  pid: number | undefined,
  signal: NodeJS.Signals = 'SIGTERM',
): void => {
  if (pid === undefined) return;

  try {
    process.kill(-pid, signal);
  } catch {
    try {
      process.kill(pid, signal);
    } catch {
      // The process has already exited.
    }
  }
};

const run = (
  command: string,
  cwd: string,
  timeoutMs: number,
  outputLimit: number,
  signal: AbortSignal | undefined,
): Promise<TerminalOutput> =>
  new Promise((resolveRun, rejectRun) => {
    if (isAborted(signal)) {
      rejectRun(abortError());
      return;
    }

    const stdout = capture();
    const stderr = capture();
    let timedOut = false;
    let aborted = false;
    let settled = false;
    let killTimer: ReturnType<typeof setTimeout> | undefined;
    let child;

    try {
      child = spawn('sh', ['-lc', command], {
        cwd,
        detached: true,
        env: environment(),
        stdio: ['ignore', 'pipe', 'pipe'],
      });
    } catch (error) {
      rejectRun(error);
      return;
    }

    const finish = (callback: () => void): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (killTimer !== undefined) clearTimeout(killTimer);
      signal?.removeEventListener('abort', onAbort);
      callback();
    };
    const stop = (): void => {
      terminate(child.pid);
      killTimer ??= setTimeout(
        () => terminate(child.pid, 'SIGKILL'),
        KILL_GRACE_MS,
      );
    };
    const onAbort = (): void => {
      aborted = true;
      stop();
    };
    const timer = setTimeout(() => {
      timedOut = true;
      stop();
    }, timeoutMs);

    child.stdout.on('data', (chunk: Buffer) =>
      append(stdout, chunk, stdout.size + stderr.size, outputLimit),
    );
    child.stderr.on('data', (chunk: Buffer) =>
      append(stderr, chunk, stdout.size + stderr.size, outputLimit),
    );
    child.once('error', (error) => finish(() => rejectRun(error)));
    child.once('close', (code) =>
      finish(() => {
        if (aborted) {
          rejectRun(abortError());
          return;
        }

        resolveRun(
          output.parse({
            exit_code: timedOut ? -1 : (code ?? -1),
            stdout: Buffer.concat(stdout.chunks).toString('utf8'),
            stderr: Buffer.concat(stderr.chunks).toString('utf8'),
            timed_out: timedOut,
            truncated: stdout.truncated || stderr.truncated,
          }),
        );
      }),
    );
    signal?.addEventListener('abort', onAbort, { once: true });

    if (isAborted(signal)) onAbort();
  });

/** Creates the benchmark's local terminal tool without a product sandbox. */
export const createTerminal = (options: {
  readonly cwd: string;
  readonly signal?: AbortSignal;
}): Tool<typeof input, typeof output> => ({
  name: 'terminal',
  description,
  input,
  output,
  definition,
  execute: async (value: TerminalInput): Promise<TerminalOutput> => {
    const cwd = await workingDirectory(options.cwd, value.working_directory);
    return run(
      value.command,
      cwd,
      value.timeout_ms ?? DEFAULT_TIMEOUT_MS,
      value.max_output_chars ?? OUTPUT_LIMIT_BYTES,
      options.signal,
    );
  },
});
