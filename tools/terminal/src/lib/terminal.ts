import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { randomUUID } from 'node:crypto';

import { createTool as defineTool } from 'tools';
import type { SandboxExecResult, SandboxSession } from 'sandbox';
import { z } from 'zod';

const DEFAULT_TIMEOUT_MS = 120_000;
const MAX_TIMEOUT_MS = 600_000;
const SHORT_OUTPUT_BYTE_LIMIT = 12_000;
const HEAD_LINE_LIMIT = 24;
const TAIL_LINE_LIMIT = 48;
const MAX_LINE_CHARS = 1000;
const DIAGNOSTIC_LINE_LIMIT = 20;
const DIAGNOSTIC_CONTEXT_RADIUS = 2;

const description =
  'Executes shell commands inside the injected sandbox session. Returns compact stdout/stderr summaries, diagnostics, exit_code, duration, and a raw_output_ref when trace storage is enabled.';

export const schema = z
  .object({
    command: z.string(),
    working_directory: z.string().optional(),
    timeout_ms: z.number().int().nonnegative().optional(),
  })
  .strict();

export type CompactStream = {
  readonly bytes: number;
  readonly lines: number;
  readonly head: readonly string[];
  readonly tail: readonly string[];
  readonly omitted_lines: number;
  readonly omitted_bytes: number;
  readonly truncated: boolean;
};

export type TerminalOutput = {
  readonly schema: 'terminal.compact.v1';
  readonly trace_id: string | null;
  readonly command: string;
  readonly working_directory: string;
  readonly exit_code: number;
  readonly duration_ms: number;
  readonly success: boolean;
  readonly stdout: CompactStream;
  readonly stderr: CompactStream;
  readonly diagnostics: readonly TerminalDiagnostic[];
  readonly truncation: {
    readonly truncated: boolean;
    readonly message: string;
  };
  readonly raw_output_ref: string | null;
  readonly trace_error?: string;
};

export type TerminalDiagnostic = {
  readonly kind: string;
  readonly stream: 'stdout' | 'stderr';
  readonly line: number;
  readonly text: string;
  readonly context: readonly {
    readonly line: number;
    readonly text: string;
  }[];
  readonly repeat_count?: number;
};

type Options = {
  readonly workspaceRoot: string;
  readonly sandbox: SandboxSession;
  readonly traceDir?: string;
};

type Execution = {
  readonly command: string;
  readonly workingDirectory: string;
  readonly stdout: string;
  readonly stderr: string;
  readonly exitCode: number;
  readonly durationMs: number;
};

/** Creates the provider-neutral sandbox terminal tool. */
export const createTool = ({ workspaceRoot, sandbox, traceDir }: Options) =>
  defineTool({
    name: 'terminal',
    description,
    schema,
    execute: (input): Promise<TerminalOutput> =>
      execute(workspaceRoot, sandbox, traceDir, input),
  });

const execute = async (
  workspaceRoot: string,
  sandbox: SandboxSession,
  traceDir: string | undefined,
  input: z.output<typeof schema>,
): Promise<TerminalOutput> => {
  const workingDirectory = resolvePath(
    workspaceRoot,
    input.working_directory?.trim() === ''
      ? '.'
      : (input.working_directory ?? '.'),
  );
  const timeoutMs = Math.min(
    input.timeout_ms ?? DEFAULT_TIMEOUT_MS,
    MAX_TIMEOUT_MS,
  );
  const execution = await runCommand(
    sandbox,
    input.command,
    workingDirectory,
    timeoutMs,
  );
  return compact(execution, await writeTrace(traceDir, execution));
};

const runCommand = async (
  sandbox: SandboxSession,
  command: string,
  workingDirectory: string,
  timeoutMs: number,
): Promise<Execution> => {
  const started = Date.now();

  try {
    const result = await sandbox.exec({
      cmd: ['sh', '-lc', command],
      cwd: workingDirectory,
      timeoutMs,
    });

    return finish(command, workingDirectory, result, started);
  } catch (error) {
    return failedExecution(command, workingDirectory, error, started);
  }
};

const finish = (
  command: string,
  workingDirectory: string,
  result: SandboxExecResult,
  started: number,
): Execution => ({
  command,
  workingDirectory,
  stdout: result.stdout,
  stderr: result.stderr,
  exitCode: result.exitCode ?? -1,
  durationMs: Date.now() - started,
});

const failedExecution = (
  command: string,
  workingDirectory: string,
  error: unknown,
  started: number,
): Execution => ({
  command,
  workingDirectory,
  stdout: '',
  stderr: `Failed to execute command in sandbox: ${errorMessage(error)}`,
  exitCode: -1,
  durationMs: Date.now() - started,
});

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error);

const compact = (
  execution: Execution,
  trace: {
    readonly traceId: string | null;
    readonly rawOutputRef: string | null;
    readonly error?: string;
  },
): TerminalOutput => {
  const stdout = compactStream(execution.stdout);
  const stderr = compactStream(execution.stderr);
  const truncated = stdout.truncated || stderr.truncated;

  return {
    schema: 'terminal.compact.v1',
    trace_id: trace.traceId,
    command: execution.command,
    working_directory: execution.workingDirectory,
    exit_code: execution.exitCode,
    duration_ms: execution.durationMs,
    success: execution.exitCode === 0,
    stdout,
    stderr,
    diagnostics: reduceDiagnostics(execution),
    truncation: {
      truncated,
      message: truncated
        ? 'terminal output compacted; full raw output is available at raw_output_ref'
        : 'terminal output fits in compact payload',
    },
    raw_output_ref: trace.rawOutputRef,
    ...(trace.error === undefined ? {} : { trace_error: trace.error }),
  };
};

const compactStream = (text: string): CompactStream => {
  const lines = splitLines(text).map(capLine);
  const stats = { bytes: Buffer.byteLength(text), lines: lines.length };

  if (stats.bytes <= SHORT_OUTPUT_BYTE_LIMIT) {
    return {
      bytes: stats.bytes,
      lines: stats.lines,
      head: lines,
      tail: [],
      omitted_lines: 0,
      omitted_bytes: 0,
      truncated: false,
    };
  }

  const head = lines.slice(0, HEAD_LINE_LIMIT);
  const tail = lines.slice(
    Math.max(lines.length - TAIL_LINE_LIMIT, HEAD_LINE_LIMIT),
  );
  const omitted = lines.slice(
    HEAD_LINE_LIMIT,
    Math.max(lines.length - TAIL_LINE_LIMIT, HEAD_LINE_LIMIT),
  );

  return {
    bytes: stats.bytes,
    lines: stats.lines,
    head,
    tail,
    omitted_lines: omitted.length,
    omitted_bytes: Buffer.byteLength(omitted.join('\n')),
    truncated: true,
  };
};

const writeTrace = async (
  traceDir: string | undefined,
  execution: Execution,
): Promise<{
  readonly traceId: string | null;
  readonly rawOutputRef: string | null;
  readonly error?: string;
}> => {
  if (traceDir === undefined) {
    return { traceId: null, rawOutputRef: null };
  }

  const traceId = randomUUID();
  const file = path.join(traceDir, `${traceId}.json`);
  try {
    await mkdir(traceDir, { recursive: true });
    await writeFile(file, JSON.stringify(execution, null, 2), 'utf8');
    return { traceId, rawOutputRef: file };
  } catch (error) {
    return {
      traceId,
      rawOutputRef: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }
};

const reduceDiagnostics = (
  execution: Execution,
): readonly TerminalDiagnostic[] => {
  const diagnostics = extractDiagnostics(execution.stdout, execution.stderr);
  return isCargoVerification(execution.command)
    ? appendCargoFailures(diagnostics, execution.stdout)
    : diagnostics;
};

const extractDiagnostics = (
  stdout: string,
  stderr: string,
): readonly TerminalDiagnostic[] =>
  [
    ...streamDiagnostics('stdout', stdout),
    ...streamDiagnostics('stderr', stderr),
  ]
    .sort(
      (left, right) =>
        diagnosticPriority(left.kind) - diagnosticPriority(right.kind) ||
        left.stream.localeCompare(right.stream) ||
        left.line - right.line,
    )
    .slice(0, DIAGNOSTIC_LINE_LIMIT);

const streamDiagnostics = (
  stream: 'stdout' | 'stderr',
  text: string,
): readonly TerminalDiagnostic[] => {
  const lines = splitLines(text);
  return lines.reduce<TerminalDiagnostic[]>((diagnostics, line, index) => {
    const kind = classifyDiagnostic(line);
    if (kind === undefined) {
      return diagnostics;
    }

    return mergeDiagnostic(diagnostics, {
      kind,
      stream,
      line: index + 1,
      text: capLine(line),
      context: contextLines(lines, index),
    });
  }, []);
};

const mergeDiagnostic = (
  diagnostics: readonly TerminalDiagnostic[],
  diagnostic: TerminalDiagnostic,
): TerminalDiagnostic[] => {
  const existing = diagnostics.findIndex(
    (value) =>
      value.kind === diagnostic.kind &&
      value.stream === diagnostic.stream &&
      value.text === diagnostic.text,
  );
  if (existing < 0) {
    return [...diagnostics, diagnostic];
  }

  return diagnostics.map((value, index) =>
    index === existing
      ? { ...value, repeat_count: (value.repeat_count ?? 1) + 1 }
      : value,
  );
};

const contextLines = (
  lines: readonly string[],
  index: number,
): TerminalDiagnostic['context'] => {
  const start = Math.max(index - DIAGNOSTIC_CONTEXT_RADIUS, 0);
  const end = Math.min(index + DIAGNOSTIC_CONTEXT_RADIUS + 1, lines.length);
  return lines.slice(start, end).map((line, offset) => ({
    line: start + offset + 1,
    text: capLine(line),
  }));
};

const classifyDiagnostic = (line: string): string | undefined => {
  const lower = line.toLowerCase();
  const checks: readonly [string, boolean][] = [
    ['rust_error', line.includes('error[E')],
    [
      'test_failure',
      lower.includes('test result: failed') ||
        lower === 'failures:' ||
        lower.includes(' failures') ||
        line.includes('FAILED'),
    ],
    [
      'cargo_error',
      lower.includes('could not compile') || lower.includes('test failed'),
    ],
    ['panic', lower.includes('panicked at') || lower.includes(' panicked')],
    [
      'exception',
      line.includes('Traceback') ||
        line.includes('Exception') ||
        line.includes('Caused by:'),
    ],
    ['typescript_error', line.includes('error TS') || /\bTS\d{4}\b/.test(line)],
    [
      'eslint_error',
      lower.includes('parsing error') ||
        lower.includes('no-unused-vars') ||
        lower.includes('problems'),
    ],
    [
      'error',
      lower.startsWith('error:') ||
        lower.includes(' error:') ||
        line.includes('ERROR') ||
        line.includes('Error:'),
    ],
    [
      'warning',
      lower.startsWith('warning:') ||
        lower.includes(' warning:') ||
        line.includes('WARN') ||
        line.includes('Warning:'),
    ],
  ];
  return checks.find(([, matches]) => matches)?.[0];
};

const diagnosticPriority = (kind: string): number =>
  kind === 'warning' ? 2 : kind === 'eslint_error' || kind === 'error' ? 1 : 0;

const appendCargoFailures = (
  diagnostics: readonly TerminalDiagnostic[],
  stdout: string,
): readonly TerminalDiagnostic[] => {
  const current = [...diagnostics];
  for (const [index, line] of splitLines(stdout).entries()) {
    if (line.startsWith('test ') && line.includes('FAILED')) {
      current.push({
        kind: 'cargo_test_failure',
        stream: 'stdout',
        line: index + 1,
        text: capLine(line),
        context: [],
      });
    }
  }
  return current;
};

const isCargoVerification = (command: string): boolean => {
  const normalized = command
    .replace(/["']/g, '')
    .replace(/\\/g, '/')
    .toLowerCase();
  return /(?:^|[\s;\/])cargo(?:\.exe)?\s+(test|check|clippy)\b/.test(
    normalized,
  );
};

const splitLines = (value: string): readonly string[] =>
  value === ''
    ? []
    : value.endsWith('\n') || value.endsWith('\r')
      ? value.trimEnd().split(/\r?\n/)
      : value.split(/\r?\n/);

const capLine = (line: string): string =>
  line.length <= MAX_LINE_CHARS
    ? line
    : `${line.slice(0, MAX_LINE_CHARS)} [line truncated; ${line.length - MAX_LINE_CHARS} chars omitted]`;

const resolvePath = (workspaceRoot: string, value: string): string =>
  path.posix.normalize(
    path.posix.isAbsolute(value)
      ? value
      : path.posix.join(workspaceRoot, value),
  );
