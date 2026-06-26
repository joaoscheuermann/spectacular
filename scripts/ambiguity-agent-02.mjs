#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { lstat, open, readdir, readFile } from 'node:fs/promises';
import { dirname, isAbsolute, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createAgent } from 'agent';
import { createCodexProvider, createFetchTransport } from 'llms';
import { createMessageStorage } from 'messages';
import { codexCredentialFromToken } from 'oauth';
import { select, text } from 'prompt-kit';
import { createTool, createToolStorage } from 'tools';
import { z } from 'zod';

const ROOT_DIR = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const ENV_PATH = resolve(ROOT_DIR, '.env');
const CODEX_AUTHORIZATION_ENV_KEY = 'CODEX_AUTHORIZATION';
const SKIPPED_DIRS = new Set([
  '.git',
  'node_modules',
  'target',
  'dist',
  'local_cache',
]);
const MAX_LIST_ENTRIES = 500;
const MAX_READ_BYTES = 100 * 1024;
const MAX_SEARCH_FILES = 1000;
const MAX_SEARCH_MATCHES = 100;
const MAX_SEARCH_FILE_BYTES = 1024 * 1024;
const MAX_LINE_LENGTH = 500;
const MAX_TERMINAL_ARGS = 40;
const MAX_TERMINAL_ARG_LENGTH = 1000;
const MAX_TERMINAL_OUTPUT_BYTES = 64 * 1024;
const DEFAULT_TERMINAL_TIMEOUT_MS = 10_000;
const MAX_TERMINAL_TIMEOUT_MS = 30_000;
const ALLOWED_TERMINAL_COMMANDS = new Set([
  'git',
  'jest',
  'node',
  'node.exe',
  'npm',
  'npm.cmd',
  'npx',
  'npx.cmd',
  'nx',
  'pnpm',
  'rg',
  'tsc',
  'tsx',
  'vitest',
  'yarn',
]);
const NODE_TERMINAL_COMMANDS = new Set(['node', 'node.exe']);
const NPM_CLI_COMMANDS = new Map([
  ['npm', 'npm-cli.js'],
  ['npm.cmd', 'npm-cli.js'],
  ['npx', 'npx-cli.js'],
  ['npx.cmd', 'npx-cli.js'],
]);
const ALLOWED_GIT_SUBCOMMANDS = new Set([
  'diff',
  'grep',
  'log',
  'ls-files',
  'rev-parse',
  'show',
  'status',
]);
const ALLOWED_GIT_GLOBAL_OPTIONS = new Set(['--no-pager', '--version', '-C']);
const FORBIDDEN_RG_OPTIONS = new Set([
  '--follow',
  '--hidden',
  '--no-ignore',
  '--no-ignore-dot',
  '--no-ignore-exclude',
  '--no-ignore-files',
  '--no-ignore-global',
  '--no-ignore-messages',
  '--no-ignore-parent',
  '--no-ignore-vcs',
  '--unrestricted',
  '-.',
  '-u',
  '-uu',
  '-uuu',
]);
const FORBIDDEN_TERMINAL_COMMANDS = new Set([
  'bash',
  'cmd',
  'cmd.exe',
  'pwsh',
  'pwsh.exe',
  'powershell',
  'powershell.exe',
  'python',
  'python.exe',
  'python3',
  'sh',
  'wscript',
  'wscript.exe',
]);
const AMBIGUITY_LINE_WINDOW_RADIUS = 2;
const AMBIGUITY_CONTEXT_CHARS = 220;
const ANSI_INVERSE = '\x1b[7m';
const ANSI_RESET = '\x1b[0m';

const HISTORY_VALUE_MAX_LENGTH = 240;

const formatHistoryValue = (value) => {
  const text = String(value ?? '')
    .replace(/\s+/g, ' ')
    .trim();

  return text.length <= HISTORY_VALUE_MAX_LENGTH
    ? text
    : `${text.slice(0, HISTORY_VALUE_MAX_LENGTH - 3)}...`;
};

const formatResolvedAmbiguityHistoryForPrompt = (history = []) => {
  const guidance =
    'Use this history only to avoid re-raising ambiguities that have already been resolved and applied. Do not use it to ignore new, remaining, or materially different ambiguities in the current prompt.';

  if (history.length === 0) {
    return ['No ambiguities have been resolved yet.', guidance].join('\n');
  }

  return [
    ...history.map(
      ({ iteration, risk, selected, applied }) =>
        `- Iteration ${iteration}: kind=${formatHistoryValue(
          risk.kind,
        )}; risk=${formatHistoryValue(risk.risk)}; reason=${formatHistoryValue(
          risk.reason,
        )}; text=${formatHistoryValue(risk.text)}; line=${formatHistoryValue(
          risk.line,
        )}; selectedLabel=${formatHistoryValue(
          selected.label,
        )}; resolution=${formatHistoryValue(
          selected.resolution,
        )}; promptChange=${formatHistoryValue(
          selected.promptChange,
        )}; appliedSummary=${formatHistoryValue(applied.changeSummary)}`,
    ),
    guidance,
  ].join('\n');
};

const createRiskAgent = (credential, resolvedAmbiguityHistory) => {
  const resolvedAmbiguityHistorySection =
    formatResolvedAmbiguityHistoryForPrompt(resolvedAmbiguityHistory);
  const SYSTEM_PROMPT = `
# Goal
Identify all "ambiguity risks" in the provided text that could lead to incorrect understanding, false assumptions, or operational mistakes by an coding agent that are not satisfied by the current codebase context.

Analyze and flag these risks in order of priority, focusing first on foundational issues (like pronoun reference) before addressing broader contextual issues.

Explore the codebase before outputting the issues.

# Priority of Analysis (Most Critical to Least Critical)

Analyze the text following this hierarchy. When presenting findings, list the high-priority risks first.

## 1. Referential Ambiguity (Priority 1)
*   **Definition:** Occurs when a pronoun (such as "he," "she," "it," "they," "this," "that") or a descriptive phrase could point to more than one possible noun (antecedent) in the text.
*   **Why it is critical:** This is a foundational blocker; if the reader cannot identify who or what is performing or receiving an action, the rest of the sentence cannot be reliably parsed.
*   **Examples:**
    *   *"Sarah told Jennifer that she won the lottery."* (Who won? Sarah or Jennifer?)
    *   *"The car hit the pole, but it wasn't damaged."* (What survived undamaged? The car or the pole?)

## 2. Syntactic (Structural) Ambiguity (Priority 2)
*   **Definition:** Occurs when the grammatical structure or word order of a sentence allows for multiple, distinct interpretations, even though the individual words are clear.
*   **Why it is critical:** Once actors are identified, structural ambiguity changes the relationship between them, altering who is doing what, or which modifiers apply to which words.
*   **Examples:**
    *   *"I saw the man with the telescope."* (Did I use a telescope to see the man, or did the man have a telescope?)
    *   *"Visiting relatives can be boring."* (Is the act of visiting relatives boring, or are the relatives who visit boring?)

## 3. Semantic Ambiguity (Priority 3)
*   **Definition:** Occurs when the meaning of a sentence is unclear due to words with multiple meanings (polysemy) or the logical scope of quantifiers (like "every," "some," "any," or "not").
*   **Why it is critical:** This affects the exact scope, conditions, limits, or definitions of the instructions, even if the structure and actors are clear.
*   **Examples:**
    *   *"All students did not pass the test."* (Does this mean zero students passed, or that some students passed but not all of them?)
    *   *"She loves her dog more than her husband."* (Does she love the dog more than she loves her husband, or does she love the dog more than her husband loves the dog?)

## 4. Pragmatic Ambiguity (Priority 4)
*   **Definition:** Occurs when the literal meaning of a sentence is straightforward, but the speaker's true intent, tone, or communicative purpose is unclear without additional context (often relying on implicit assumptions).
*   **Why it is critical:** This addresses the gap between what is literally written and what is actually intended. Resolving Priorities 1–3 often eliminates most pragmatic ambiguity.
*   **Examples:**
    *   *"It's getting late."* (Is this a simple statement of time, or is it an indirect request for guests to leave?)
    *   *"Can you reach the top shelf?"* (Is this an inquiry about physical capability, or is it an indirect, polite request to grab something?)

# Resolved ambiguity history
<resolved_ambiguity_history>
${resolvedAmbiguityHistorySection}
</resolved_ambiguity_history>
`;

  return createAgent({
    provider: createCodexProvider({
      transport: createFetchTransport(),
      authorization: credential.authorization,
      chatGptAccountId: credential.accountId,
      fedramp: credential.fedramp,
    }),
    tools: createToolStorage(createCodebaseTools()),
    messages: createMessageStorage(),
    system: SYSTEM_PROMPT,
    model: 'gpt-5.5',
  });
};

// const TARGET_PROMPT = readFileSync(
//   `C:\\Users\\jvito\\Documents\\git\\spectacular\\doric\\docs\\00-references\\01-source-prompt.md`,
//   {
//     encoding: 'utf-8',
//   },
// );
const TARGET_PROMPT =
  'Fix the issue in the config package: FAIL packages/config/tests/config.test.ts, failing test: "returns config data when message metadata contains configuration"';

const riskSchema = z.object({
  risk: z.string(),
  kind: z.enum(['semantic', 'syntactic', 'pragmatic', 'referential']),
  reason: z.string(),
  text: z.string(),
  line: z.number(),
});

const riskDetectionSchema = z.object({
  risks: z.array(riskSchema),
});

const solutionOptionSchema = z.object({
  label: z.string(),
  resolution: z.string(),
  promptChange: z.string(),
});

const optionsSchema = z.object({
  inputType: z.enum(['select', 'text']),
  question: z.string(),
  guidance: z.string(),
  placeholder: z.string(),
  options: z.array(solutionOptionSchema),
});

const rewriteSchema = z.object({
  prompt: z.string(),
  changeSummary: z.string(),
});

const readEnvFile = async () => {
  try {
    return await readFile(ENV_PATH, 'utf8');
  } catch (error) {
    if (error?.code === 'ENOENT') {
      throw new Error('Root .env is required.');
    }

    throw error;
  }
};

const parseEnv = (contents) =>
  Object.fromEntries(
    contents
      .split(/\r?\n/)
      .map(parseEnvLine)
      .filter((entry) => entry !== undefined),
  );

const parseEnvLine = (line) => {
  const trimmed = line.trim();

  if (trimmed === '' || trimmed.startsWith('#')) {
    return undefined;
  }

  const normalized = trimmed.startsWith('export ')
    ? trimmed.slice('export '.length)
    : line;
  const separator = normalized.indexOf('=');

  if (separator < 1) {
    return undefined;
  }

  const key = normalized.slice(0, separator).trim();
  const value = normalized.slice(separator + 1).trim();

  if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(key)) {
    return undefined;
  }

  return [key, unquote(key, value)];
};

const unquote = (key, value) => {
  if (value.startsWith('"') && value.endsWith('"')) {
    try {
      return JSON.parse(value);
    } catch (cause) {
      throw new Error(`${key} has an invalid quoted value.`, { cause });
    }
  }

  if (value.startsWith("'") && value.endsWith("'")) {
    return value.slice(1, -1);
  }

  return value;
};

const required = (env, key) => {
  const value = env[key]?.trim();

  if (value === undefined || value === '') {
    throw new Error(`${key} is required in root .env.`);
  }

  return value;
};

const listCodebaseFilesTool = () =>
  createTool({
    name: 'list_codebase_files',
    description:
      'List files and directories under a repo-relative directory. Skips heavyweight and secret-like paths.',
    schema: z
      .object({
        path: z.string().optional(),
        limit: z.number().int().min(1).max(1000).optional(),
      })
      .strict(),
    execute: (args = {}) =>
      logToolExecution('list_codebase_files', args, () =>
        listCodebaseFiles(args.path ?? '.', args.limit ?? MAX_LIST_ENTRIES),
      ),
  });

const readCodebaseFileTool = () =>
  createTool({
    name: 'read_codebase_file',
    description:
      'Read a UTF-8 text file by repo-relative path. Refuses env, secret-like, excluded, and symlink paths.',
    schema: z
      .object({
        path: z.string().min(1),
        maxBytes: z.number().int().min(1).max(MAX_READ_BYTES).optional(),
      })
      .strict(),
    execute: (args = {}) =>
      logToolExecution('read_codebase_file', args, () =>
        readCodebaseFile(args.path, args.maxBytes ?? MAX_READ_BYTES),
      ),
  });

const searchCodebaseTextTool = () =>
  createTool({
    name: 'search_codebase_text',
    description:
      'Search literal text across files under a repo-relative path. Skips heavyweight and secret-like paths.',
    schema: z
      .object({
        path: z.string().optional(),
        query: z.string().min(1),
        ignoreCase: z.boolean().optional(),
        limit: z.number().int().min(1).max(500).optional(),
      })
      .strict(),
    execute: (args = {}) =>
      logToolExecution('search_codebase_text', args, () =>
        searchCodebaseText(
          args.path ?? '.',
          args.query,
          args.ignoreCase ?? false,
          args.limit ?? MAX_SEARCH_MATCHES,
        ),
      ),
  });

const runTerminalCommandTool = () =>
  createTool({
    name: 'run_terminal_command',
    description:
      'Run a bounded local developer command in a repo-relative working directory for this test environment. Uses shell-free execution, sanitized environment variables, timeout, and capped output.',
    schema: z
      .object({
        command: z.string().min(1).max(200),
        args: z
          .array(z.string().max(MAX_TERMINAL_ARG_LENGTH))
          .max(MAX_TERMINAL_ARGS)
          .optional(),
        cwd: z.string().optional(),
        timeoutMs: z
          .number()
          .int()
          .min(1_000)
          .max(MAX_TERMINAL_TIMEOUT_MS)
          .optional(),
        maxOutputBytes: z
          .number()
          .int()
          .min(1_000)
          .max(MAX_TERMINAL_OUTPUT_BYTES)
          .optional(),
      })
      .strict(),
    execute: (args = {}) =>
      logToolExecution('run_terminal_command', args, () =>
        runTerminalCommand({
          command: args.command,
          args: args.args ?? [],
          cwd: args.cwd ?? '.',
          timeoutMs: args.timeoutMs ?? DEFAULT_TERMINAL_TIMEOUT_MS,
          maxOutputBytes: args.maxOutputBytes ?? MAX_TERMINAL_OUTPUT_BYTES,
        }),
      ),
  });

const createCodebaseTools = () => [
  listCodebaseFilesTool(),
  readCodebaseFileTool(),
  searchCodebaseTextTool(),
  runTerminalCommandTool(),
];

const logToolExecution = async (name, args, execute) => {
  console.log(`[tool:${name}] call`, summarizeToolCall(name, args));

  try {
    const result = await execute();
    console.log(`[tool:${name}] result`, summarizeToolResult(name, result));
    return result;
  } catch (error) {
    console.log(`[tool:${name}] error`, errorMessage(error));
    throw error;
  }
};

const summarizeToolCall = (name, args) => {
  if (name === 'run_terminal_command') {
    return {
      command: args.command,
      argCount: args.args?.length ?? 0,
      cwd: args.cwd ?? '.',
      timeoutMs: args.timeoutMs ?? DEFAULT_TERMINAL_TIMEOUT_MS,
      maxOutputBytes: args.maxOutputBytes ?? MAX_TERMINAL_OUTPUT_BYTES,
    };
  }

  return args;
};

const summarizeToolResult = (name, result) => {
  if (name === 'list_codebase_files') {
    return {
      error: result.error,
      root: result.root,
      total: result.total,
      truncated: result.truncated,
      entries: result.entries.slice(0, 10),
    };
  }

  if (name === 'read_codebase_file') {
    return {
      error: result.error,
      path: result.path,
      bytes: result.bytes,
      truncated: result.truncated,
      contentChars: result.content?.length,
    };
  }

  if (name === 'search_codebase_text') {
    return {
      error: result.error,
      total: result.total,
      filesSearched: result.filesSearched,
      truncated: result.truncated,
      matches: result.matches.slice(0, 10).map(({ path, line, column }) => ({
        path,
        line,
        column,
      })),
    };
  }

  if (name === 'run_terminal_command') {
    return {
      error: result.error,
      command: result.command,
      cwd: result.cwd,
      exitCode: result.exitCode,
      signal: result.signal,
      timedOut: result.timedOut,
      stdoutBytes: result.stdoutBytes,
      stderrBytes: result.stderrBytes,
      stdoutTruncated: result.stdoutTruncated,
      stderrTruncated: result.stderrTruncated,
    };
  }

  return result;
};

const listCodebaseFiles = async (path, limit) => {
  const root = await inspectRepoPath(path);

  if (root.error !== undefined) {
    return { error: root.error, entries: [], total: 0, truncated: false };
  }

  if (root.kind !== 'directory') {
    return {
      error: `Path is not a directory: ${path}`,
      entries: [],
      total: 0,
      truncated: false,
    };
  }

  const state = { entries: [], truncated: false };
  await appendEntries(root.absolute, state, limit);

  return {
    root: root.relative,
    entries: state.entries,
    total: state.entries.length,
    truncated: state.truncated,
  };
};

const readCodebaseFile = async (path, maxBytes) => {
  const target = await inspectRepoPath(path);

  if (target.error !== undefined) {
    return { error: target.error };
  }

  if (target.kind !== 'file') {
    return { error: `Path is not a file: ${path}` };
  }

  const handle = await open(target.absolute, 'r');

  try {
    const buffer = Buffer.alloc(maxBytes);
    const { bytesRead } = await handle.read(buffer, 0, maxBytes, 0);

    return {
      path: target.relative,
      content: buffer.subarray(0, bytesRead).toString('utf8'),
      encoding: 'utf8',
      bytes: target.size,
      truncated: target.size > bytesRead,
    };
  } finally {
    await handle.close();
  }
};

const searchCodebaseText = async (path, query, ignoreCase, limit) => {
  const root = await inspectRepoPath(path);

  if (root.error !== undefined) {
    return emptySearch(root.error);
  }

  if (root.kind !== 'file' && root.kind !== 'directory') {
    return emptySearch(`Path is not searchable: ${path}`);
  }

  const files =
    root.kind === 'file'
      ? [root]
      : await collectSearchFiles(root.absolute, MAX_SEARCH_FILES);
  const normalizedQuery = ignoreCase ? query.toLocaleLowerCase() : query;
  const matches = [];
  let filesSearched = 0;
  let truncated = files.length >= MAX_SEARCH_FILES;

  for (const file of files) {
    if (matches.length >= limit) {
      truncated = true;
      break;
    }

    if (file.size > MAX_SEARCH_FILE_BYTES) {
      continue;
    }

    const text = await readFile(file.absolute, 'utf8').catch(() => undefined);
    if (text === undefined || text.includes('\u0000')) {
      continue;
    }

    filesSearched += 1;
    const lines = text.split(/\r?\n/);

    for (const [index, line] of lines.entries()) {
      const haystack = ignoreCase ? line.toLocaleLowerCase() : line;
      const column = haystack.indexOf(normalizedQuery);

      if (column < 0) {
        continue;
      }

      matches.push({
        path: file.relative,
        line: index + 1,
        column: column + 1,
        text: truncateLine(line),
      });

      if (matches.length >= limit) {
        truncated = true;
        break;
      }
    }
  }

  return {
    matches,
    total: matches.length,
    filesSearched,
    truncated,
  };
};

const runTerminalCommand = async ({
  command,
  args,
  cwd,
  timeoutMs,
  maxOutputBytes,
}) => {
  const executable = inspectTerminalCommand(command, args);

  if (executable.error !== undefined) {
    return { error: executable.error };
  }

  const root = await inspectRepoPath(cwd);

  if (root.error !== undefined) {
    return { error: root.error };
  }

  if (root.kind !== 'directory') {
    return { error: `Working directory is not a directory: ${cwd}` };
  }

  const symlink = await findSymlinkPathSegment(root.relative);

  if (symlink !== undefined) {
    return { error: `Working directory includes symbolic link: ${symlink}` };
  }

  const result = await spawnTerminalCommand({
    command: executable.command,
    args: executable.args,
    cwd: root.absolute,
    timeoutMs,
    maxOutputBytes,
  });

  return {
    ...result,
    command: executable.displayCommand,
    args,
    cwd: root.relative,
  };
};

const inspectTerminalCommand = (command, args) => {
  const value = command.trim();
  const normalized = value.toLocaleLowerCase();

  if (value === '') {
    return { error: 'Command is required.' };
  }

  if (isAbsolute(value) || value.includes('/') || value.includes('\\')) {
    return { error: 'Command must be an executable name, not a path.' };
  }

  if (value.includes('\u0000')) {
    return { error: 'Command must not contain null bytes.' };
  }

  if (FORBIDDEN_TERMINAL_COMMANDS.has(normalized)) {
    return { error: `Command is not allowed: ${value}` };
  }

  if (!ALLOWED_TERMINAL_COMMANDS.has(normalized)) {
    return { error: `Command is not allowed: ${value}` };
  }

  const argsError = inspectTerminalArgs(normalized, args);

  if (argsError !== undefined) {
    return { error: argsError };
  }

  return resolveTerminalExecutable(value, normalized, args);
};

const resolveTerminalExecutable = (command, normalized, args) => {
  if (NODE_TERMINAL_COMMANDS.has(normalized)) {
    return {
      command: process.execPath,
      args,
      displayCommand: command,
    };
  }

  const npmCli = resolveNpmCliPath(normalized);

  if (npmCli.error !== undefined) {
    return npmCli;
  }

  if (npmCli.path !== undefined) {
    return {
      command: process.execPath,
      args: [npmCli.path, ...args],
      displayCommand: command,
    };
  }

  return {
    command,
    args,
    displayCommand: command,
  };
};

const resolveNpmCliPath = (command) => {
  const cli = NPM_CLI_COMMANDS.get(command);

  if (cli === undefined) {
    return {};
  }

  const path = resolve(
    dirname(process.execPath),
    'node_modules',
    'npm',
    'bin',
    cli,
  );

  if (existsSync(path)) {
    return { path };
  }

  return { error: `Unable to find npm CLI entrypoint for ${command}.` };
};

const inspectTerminalArgs = (command, args) => {
  const invalidArg = args.find((arg) => arg.includes('\u0000'));

  if (invalidArg !== undefined) {
    return 'Command arguments must not contain null bytes.';
  }

  const unsafeArg = args.find(isUnsafeTerminalArg);

  if (unsafeArg !== undefined) {
    return `Command argument references an excluded path: ${unsafeArg}`;
  }

  if (command === 'git') {
    return inspectGitArgs(args);
  }

  if (command === 'rg') {
    return inspectRgArgs(args);
  }

  return undefined;
};

const inspectRgArgs = (args) => {
  const forbidden = args.find(isForbiddenRgArg);

  if (forbidden !== undefined) {
    return `rg option is not allowed: ${forbidden}`;
  }

  return undefined;
};

const isForbiddenRgArg = (arg) =>
  FORBIDDEN_RG_OPTIONS.has(arg) ||
  (arg.startsWith('-') && !arg.startsWith('--') && arg.includes('u'));

const inspectGitArgs = (args) => {
  const subcommand = findGitSubcommand(args);

  if (subcommand === undefined) {
    return 'Git command must include an allowed inspection subcommand.';
  }

  if (!ALLOWED_GIT_SUBCOMMANDS.has(subcommand)) {
    return `Git subcommand is not allowed: ${subcommand}`;
  }

  return undefined;
};

const findGitSubcommand = (args) => {
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];

    if (arg === '-C') {
      index += 1;
      continue;
    }

    if (ALLOWED_GIT_GLOBAL_OPTIONS.has(arg)) {
      continue;
    }

    if (arg.startsWith('-')) {
      return undefined;
    }

    return arg;
  }

  return undefined;
};

const isUnsafeTerminalArg = (arg) =>
  extractPathCandidates(arg).some(isUnsafeTerminalPath);

const extractPathCandidates = (arg) =>
  arg
    .split(/[=:,]/)
    .map((part) => part.trim())
    .filter((part) => part !== '');

const isUnsafeTerminalPath = (value) => {
  const normalized = value.replaceAll('\\', '/');

  if (isAbsolute(normalized)) {
    return true;
  }

  const parts = normalized.split('/').filter(Boolean);

  if (parts.includes('..')) {
    return true;
  }

  const candidate = parts.join('/');

  if (candidate === '') {
    return false;
  }

  if (isSkippedDir(candidate)) {
    return true;
  }

  return isPathLikeTerminalArg(candidate) && isSecretLikePath(candidate);
};

const isPathLikeTerminalArg = (value) =>
  value.includes('/') ||
  value.startsWith('.') ||
  /\.[A-Za-z0-9]+$/.test(value) ||
  /^id_(?:rsa|dsa|ecdsa|ed25519)$/i.test(value);

const findSymlinkPathSegment = async (path) => {
  if ((await lstat(ROOT_DIR)).isSymbolicLink()) {
    return '.';
  }

  const parts = path === '.' ? [] : path.split('/').filter(Boolean);
  let current = ROOT_DIR;

  for (const part of parts) {
    current = resolve(current, part);

    if ((await lstat(current)).isSymbolicLink()) {
      return toRepoRelative(current);
    }
  }

  return undefined;
};

const spawnTerminalCommand = ({
  command,
  args,
  cwd,
  timeoutMs,
  maxOutputBytes,
}) =>
  new Promise((resolveResult) => {
    const stdout = createOutputCapture(maxOutputBytes);
    const stderr = createOutputCapture(maxOutputBytes);
    let timedOut = false;

    const child = spawn(command, args, {
      cwd,
      env: sanitizeEnv(process.env),
      shell: false,
      windowsHide: true,
    });
    const timeout = setTimeout(() => {
      timedOut = true;
      child.kill();
    }, timeoutMs);

    child.stdout?.on('data', (chunk) => stdout.append(chunk));
    child.stderr?.on('data', (chunk) => stderr.append(chunk));
    child.on('error', (error) => {
      clearTimeout(timeout);
      resolveResult({
        error: errorMessage(error),
        stdout: stdout.text(),
        stderr: stderr.text(),
        stdoutBytes: stdout.bytes,
        stderrBytes: stderr.bytes,
        stdoutTruncated: stdout.truncated,
        stderrTruncated: stderr.truncated,
        timedOut,
      });
    });
    child.on('close', (exitCode, signal) => {
      clearTimeout(timeout);
      resolveResult({
        exitCode,
        signal,
        timedOut,
        stdout: stdout.text(),
        stderr: stderr.text(),
        stdoutBytes: stdout.bytes,
        stderrBytes: stderr.bytes,
        stdoutTruncated: stdout.truncated,
        stderrTruncated: stderr.truncated,
      });
    });
  });

const createOutputCapture = (maxBytes) => {
  const chunks = [];
  let bytes = 0;
  let truncated = false;

  return {
    get bytes() {
      return bytes;
    },
    get truncated() {
      return truncated;
    },
    append(chunk) {
      const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      const remaining = maxBytes - bytes;

      if (remaining <= 0) {
        truncated = true;
        return;
      }

      const next =
        buffer.length <= remaining ? buffer : buffer.subarray(0, remaining);
      chunks.push(next);
      bytes += next.length;
      truncated = truncated || next.length < buffer.length;
    },
    text() {
      const suffix = truncated ? '\n[truncated]' : '';
      return `${Buffer.concat(chunks).toString('utf8')}${suffix}`;
    },
  };
};

const sanitizeEnv = (env) =>
  Object.fromEntries(
    Object.entries(env).filter(([key]) => !isSecretLikeEnvKey(key)),
  );

const isSecretLikeEnvKey = (key) =>
  key === CODEX_AUTHORIZATION_ENV_KEY ||
  /(?:^|_)(?:auth|authorization|bearer|credential|credentials|key|password|secret|token|tokens)(?:_|$)/i.test(
    key,
  );

const appendEntries = async (directory, state, limit) => {
  if (state.truncated) {
    return;
  }

  const entries = await readdir(directory, { withFileTypes: true });

  for (const entry of entries.sort((left, right) =>
    left.name.localeCompare(right.name),
  )) {
    if (state.entries.length >= limit) {
      state.truncated = true;
      return;
    }

    const absolute = resolve(directory, entry.name);
    const repoRelative = toRepoRelative(absolute);

    if (entry.isDirectory()) {
      if (isSkippedDir(repoRelative)) {
        continue;
      }

      state.entries.push({ path: repoRelative, type: 'directory' });
      await appendEntries(absolute, state, limit);
      continue;
    }

    if (entry.isFile() && !isSecretLikePath(repoRelative)) {
      state.entries.push({ path: repoRelative, type: 'file' });
      continue;
    }

    if (entry.isSymbolicLink()) {
      state.entries.push({ path: repoRelative, type: 'symlink' });
    }
  }
};

const collectSearchFiles = async (directory, limit) => {
  const files = [];
  await appendSearchFiles(directory, files, limit);
  return files;
};

const appendSearchFiles = async (directory, files, limit) => {
  if (files.length >= limit) {
    return;
  }

  const entries = await readdir(directory, { withFileTypes: true });

  for (const entry of entries.sort((left, right) =>
    left.name.localeCompare(right.name),
  )) {
    if (files.length >= limit) {
      return;
    }

    const absolute = resolve(directory, entry.name);
    const repoRelative = toRepoRelative(absolute);

    if (entry.isDirectory()) {
      if (!isSkippedDir(repoRelative)) {
        await appendSearchFiles(absolute, files, limit);
      }

      continue;
    }

    if (entry.isFile() && !isSecretLikePath(repoRelative)) {
      const stats = await lstat(absolute);
      files.push({
        absolute,
        relative: repoRelative,
        size: stats.size,
        kind: 'file',
      });
    }
  }
};

const inspectRepoPath = async (path) => {
  const resolved = resolveRepoPath(path);

  if (resolved.error !== undefined) {
    return resolved;
  }

  const stats = await lstat(resolved.absolute).catch((error) => {
    if (error?.code === 'ENOENT') {
      return undefined;
    }

    throw error;
  });

  if (stats === undefined) {
    return { error: `Path not found: ${path}` };
  }

  if (stats.isSymbolicLink()) {
    return { error: `Refusing to follow symbolic link: ${path}` };
  }

  if (isSkippedDir(resolved.relative) || isSecretLikePath(resolved.relative)) {
    return { error: `Path is excluded: ${path}` };
  }

  if (stats.isDirectory()) {
    return { ...resolved, kind: 'directory', size: stats.size };
  }

  if (stats.isFile()) {
    return { ...resolved, kind: 'file', size: stats.size };
  }

  return { ...resolved, kind: 'other', size: stats.size };
};

const resolveRepoPath = (path) => {
  const value = path.trim() === '' ? '.' : path.trim();

  if (isAbsolute(value)) {
    return { error: `Path must be relative to the repo root: ${path}` };
  }

  const absolute = resolve(ROOT_DIR, value);
  const repoRelative = relative(ROOT_DIR, absolute);

  if (repoRelative.startsWith('..') || isAbsolute(repoRelative)) {
    return { error: `Path escapes repo root: ${path}` };
  }

  return {
    absolute,
    relative: toRepoRelative(absolute),
  };
};

const isSkippedDir = (path) =>
  path
    .split('/')
    .filter(Boolean)
    .some((part) => SKIPPED_DIRS.has(part));

const isSecretLikePath = (path) => {
  const name = path.split('/').at(-1)?.toLocaleLowerCase() ?? '';

  return (
    name === '.env' ||
    name.startsWith('.env.') ||
    name.endsWith('.env') ||
    name.includes('.env.') ||
    name === 'id_rsa' ||
    name === 'id_dsa' ||
    name === 'id_ecdsa' ||
    name === 'id_ed25519' ||
    /\.(?:key|pem|p12|pfx)$/.test(name) ||
    /(?:^|[._-])(?:secret|secrets|credential|credentials|token|tokens)(?:[._-]|$)/.test(
      name,
    )
  );
};

const toRepoRelative = (absolute) =>
  relative(ROOT_DIR, absolute).split(sep).filter(Boolean).join('/') || '.';

const truncateLine = (line) =>
  line.length <= MAX_LINE_LENGTH
    ? line
    : `${line.slice(0, MAX_LINE_LENGTH)}... [truncated]`;

const emptySearch = (error) => ({
  error,
  matches: [],
  total: 0,
  filesSearched: 0,
  truncated: false,
});

const createSolutionsAgent = (credential) => {
  const tools = createCodebaseTools();

  return createAgent({
    provider: createCodexProvider({
      transport: createFetchTransport(),
      authorization: credential.authorization,
      chatGptAccountId: credential.accountId,
      fedramp: credential.fedramp,
    }),
    tools: createToolStorage(tools),
    messages: createMessageStorage(),
    system: `
      Given the prompt and the issue provided, decide how to ask the user for the missing ambiguity resolution.

      Do not rewrite the prompt. Do not ask meta questions about rewriting strategy, wording preferences, or process.

      Use inputType "select" when the ambiguity has a small set of concrete known solutions. For select prompts, produce concrete options the user can select, and include the exact prompt change each option implies.

      Use inputType "text" when the user must provide missing domain-specific wording, names, exact constraints, or custom content that cannot be enumerated reliably. For text prompts, provide a direct user-facing question plus enough guidance for the user to answer. Return an empty options array for text prompts.

      Before defining a solution, the answer might lie inside the current codebase. Use the codebase tools and the terminal tool to inspect the local repository when repository context can clarify the intended resolution.
    `,
    model: 'gpt-5.5',
  });
};

const createRewriteAgent = (credential) =>
  createAgent({
    provider: createCodexProvider({
      transport: createFetchTransport(),
      authorization: credential.authorization,
      chatGptAccountId: credential.accountId,
      fedramp: credential.fedramp,
    }),
    tools: createToolStorage([]),
    messages: createMessageStorage(),
    system: `
      Rewrite the current prompt by applying exactly the selected ambiguity resolution.
      Preserve all unrelated wording, ordering, and requirements.
      Return the full rewritten prompt and a concise summary of the applied change.
      Do not ask questions, inspect the codebase, or alter unrelated requirements.
    `,
    model: 'gpt-5.5',
  });

const detectRisk = async (
  credential,
  prompt,
  iteration,
  resolvedAmbiguityHistory,
) => {
  const input = `
    # Task
    Analyze the current prompt using the ambiguity risk priority rules.
    Return only the first or highest-priority remaining ambiguity risk.
    If there are no ambiguity risks left, return an empty risks array.

    # Current prompt
    <current_prompt>
    ${prompt}
    </current_prompt>
  `;

  console.log(`[loop:${iteration}] risk detection call`, {
    promptChars: prompt.length,
    resolvedAmbiguityHistoryCount: resolvedAmbiguityHistory.length,
  });

  const response = await createRiskAgent(
    credential,
    resolvedAmbiguityHistory,
  ).complete(input, {
    schema: riskDetectionSchema,
  });
  const risks = response.structured.risks;
  const risk = risks[0];

  console.log(`[loop:${iteration}] risk detection result`, {
    finishReason: response.finishReason,
    riskCount: risks.length,
    selectedRisk: risk,
  });

  return risk;
};

const extractOptions = async (credential, prompt, risk, iteration) => {
  const input = `
    # Task
    Extract the user input needed to resolve the ambiguity.
    Do not ask about rewriting strategy, wording preferences, or process.

    Use inputType "select" when the ambiguity has a small set of concrete known solutions.
    - Return one user-facing question.
    - Return one or more concrete options.
    - Each option must identify a specific intended interpretation and an exact prompt change.

    Use inputType "text" when the user must provide missing domain-specific wording, names, exact constraints, or custom content that cannot be enumerated reliably.
    - Return one direct user-facing question.
    - Use guidance and placeholder to explain the kind of answer needed.
    - Return an empty options array.

    # Current prompt
    <current_prompt>
    ${prompt}
    </current_prompt>

    # Ambiguity risk
    ${formatRiskForPrompt(risk)}
  `;

  console.log(`[loop:${iteration}] options call`, {
    risk: risk.risk,
    kind: risk.kind,
  });

  const response = await createSolutionsAgent(credential).complete(input, {
    schema: optionsSchema,
  });
  const result = response.structured;

  console.log(`[loop:${iteration}] options result`, {
    finishReason: response.finishReason,
    inputType: result.inputType,
    question: result.question,
    guidance: result.guidance,
    placeholder: result.placeholder,
    options: result.options,
  });

  if (result.inputType === 'select' && result.options.length === 0) {
    throw new Error('Options agent returned no selectable options.');
  }

  return result;
};

const chooseOption = async (prompt, risk, request, iteration) => {
  console.log(`[loop:${iteration}] user prompt`, {
    title: `Resolve ambiguity ${iteration}`,
    inputType: request.inputType,
    question: request.question,
    guidance: request.guidance,
    placeholder: request.placeholder,
    choices: request.options,
  });

  renderAmbiguityContext(prompt, risk, iteration);

  if (request.inputType === 'select') {
    const choices = request.options.map((option, index) => ({
      key: `${index + 1}. ${truncateChoiceLabel(option.label)}`,
      value: option,
    }));

    console.log(`[loop:${iteration}] rendering select prompt`, {
      question: request.question,
      choiceCount: choices.length,
    });

    const decision = await select(
      `Resolve ambiguity ${iteration}`,
      request.question,
      choices,
    );

    console.log(`[loop:${iteration}] user decision`, {
      inputType: request.inputType,
      decision,
    });

    return decision;
  }

  console.log(`[loop:${iteration}] rendering text prompt`, {
    question: request.question,
    guidance: request.guidance,
    placeholder: request.placeholder,
  });

  const answer = (await text(formatTextQuestion(request))).trim();

  if (answer === '') {
    throw new Error('Text prompt requires a non-empty answer.');
  }

  const decision = {
    label: 'Custom text answer',
    resolution: answer,
    promptChange: answer,
  };

  console.log(`[loop:${iteration}] user decision`, {
    inputType: request.inputType,
    decision,
  });

  return decision;
};

const renderAmbiguityContext = (prompt, risk, iteration) => {
  const context = createAmbiguityContext(prompt, risk);

  console.log(`[loop:${iteration}] rendering ambiguity context`, {
    reportedLine: risk.line,
    ambiguousText: risk.text,
    matchedLine: context.match?.line,
    matchedColumn: context.match?.column,
    matchFound: context.match !== undefined,
    matchSource: context.match?.source,
    excerptBounds: context.groups.map(({ start, end }) => ({ start, end })),
  });

  const locator =
    context.match === undefined
      ? `reported line ${risk.line}; no exact match`
      : `reported line ${risk.line}; matched line ${context.match.line}, column ${context.match.column}`;

  console.log(`\nAmbiguity context (${locator}; highlighted as [[...]]):`);
  console.log(context.excerpt);
  console.log('');
};

const createAmbiguityContext = (prompt, risk) => {
  const lines = splitPromptLines(prompt);
  const reportedLineIndex = toLineIndex(risk.line);
  const match = locateAmbiguousText(
    prompt,
    lines,
    risk.text,
    reportedLineIndex,
  );

  if (match === undefined) {
    return createMissingAmbiguityContext(lines, risk, reportedLineIndex);
  }

  const groups = [createExcerptGroup(prompt, match)];

  return {
    match,
    groups,
    excerpt: formatContextGroups(prompt, groups),
  };
};

const splitPromptLines = (prompt) => {
  const lines = [];
  let start = 0;

  for (let index = 0; index < prompt.length; index += 1) {
    const character = prompt[index];

    if (character !== '\n' && character !== '\r') {
      continue;
    }

    const end = index;

    if (character === '\r' && prompt[index + 1] === '\n') {
      index += 1;
    }

    const lineEnd = index + 1;
    lines.push(createPromptLine(lines.length, prompt, start, end, lineEnd));
    start = lineEnd;
  }

  lines.push(
    createPromptLine(lines.length, prompt, start, prompt.length, prompt.length),
  );

  return lines;
};

const createPromptLine = (index, prompt, start, end, lineEnd) => ({
  number: index + 1,
  start,
  end,
  lineEnd,
  text: prompt.slice(start, end),
});

const toLineIndex = (line) =>
  Number.isFinite(line) ? Math.trunc(line) - 1 : undefined;

const locateAmbiguousText = (prompt, lines, text, reportedLineIndex) => {
  if (text === '') {
    return undefined;
  }

  const reportedLine = lines[reportedLineIndex];
  const reportedLineMatch =
    reportedLine === undefined
      ? undefined
      : findTextInBounds(
          prompt,
          lines,
          text,
          reportedLine.start,
          reportedLine.end,
          'reported-line',
        );

  if (reportedLineMatch !== undefined) {
    return reportedLineMatch;
  }

  const windowBounds = createLineWindowBounds(lines, reportedLineIndex);
  const windowMatch =
    windowBounds === undefined
      ? undefined
      : findTextInBounds(
          prompt,
          lines,
          text,
          windowBounds.start,
          windowBounds.end,
          'line-window',
        );

  if (windowMatch !== undefined) {
    return windowMatch;
  }

  const fullPromptIndex = prompt.indexOf(text);
  return fullPromptIndex < 0
    ? undefined
    : createAmbiguousTextMatch(lines, text, fullPromptIndex, 'full-prompt');
};

const findTextInBounds = (prompt, lines, text, start, end, source) => {
  const index = prompt.slice(start, end).indexOf(text);
  return index < 0
    ? undefined
    : createAmbiguousTextMatch(lines, text, start + index, source);
};

const createAmbiguousTextMatch = (lines, text, start, source) => {
  const line = findLineAtOffset(lines, start);

  return {
    source,
    start,
    end: start + text.length,
    line: line?.number,
    column: line === undefined ? undefined : start - line.start + 1,
  };
};

const findLineAtOffset = (lines, offset) =>
  lines.find(({ start, lineEnd }) => offset >= start && offset < lineEnd) ??
  lines.at(-1);

const createLineWindowBounds = (lines, centerIndex) => {
  if (!isValidLineIndex(lines, centerIndex)) {
    return undefined;
  }

  const firstIndex = clamp(
    centerIndex - AMBIGUITY_LINE_WINDOW_RADIUS,
    0,
    lines.length - 1,
  );
  const lastIndex = clamp(
    centerIndex + AMBIGUITY_LINE_WINDOW_RADIUS,
    0,
    lines.length - 1,
  );

  return {
    start: lines[firstIndex].start,
    end: lines[lastIndex].end,
    firstLine: lines[firstIndex].number,
    lastLine: lines[lastIndex].number,
  };
};

const isValidLineIndex = (lines, index) =>
  Number.isInteger(index) && index >= 0 && index < lines.length;

const createExcerptGroup = (prompt, match) => ({
  start: clamp(match.start - AMBIGUITY_CONTEXT_CHARS, 0, prompt.length),
  end: clamp(match.end + AMBIGUITY_CONTEXT_CHARS, 0, prompt.length),
  match,
});

const createMissingAmbiguityContext = (lines, risk, reportedLineIndex) => {
  const nearby = createNearbyLineExcerpt(lines, reportedLineIndex);
  const lineRange =
    lines.length === 0 ? 'no lines available' : `1-${lines.length}`;
  const lineStatus = isValidLineIndex(lines, reportedLineIndex)
    ? `${risk.line}`
    : `${risk.line} (outside prompt line range ${lineRange})`;
  const nearbyText =
    nearby === undefined
      ? 'Nearby lines: none available.'
      : `Nearby lines ${nearby.firstLine}-${nearby.lastLine}:\n${nearby.text}`;

  return {
    match: undefined,
    groups:
      nearby === undefined ? [] : [{ start: nearby.start, end: nearby.end }],
    excerpt: [
      'No exact match found for the reported ambiguity text.',
      `Reported line: ${lineStatus}`,
      `Ambiguous text: ${risk.text}`,
      nearbyText,
    ].join('\n'),
  };
};

const createNearbyLineExcerpt = (lines, reportedLineIndex) => {
  if (lines.length === 0) {
    return undefined;
  }

  const centerIndex = isValidLineIndex(lines, reportedLineIndex)
    ? reportedLineIndex
    : clamp(reportedLineIndex ?? 0, 0, lines.length - 1);
  const bounds = createLineWindowBounds(lines, centerIndex);

  if (bounds === undefined) {
    return undefined;
  }

  return {
    ...bounds,
    text: lines
      .slice(bounds.firstLine - 1, bounds.lastLine)
      .map((line) => `${line.number}: ${truncateLine(line.text)}`)
      .join('\n'),
  };
};

const formatContextGroups = (prompt, groups) =>
  groups
    .map((group, index) => {
      const previous = groups[index - 1];
      const omitted =
        previous === undefined
          ? group.start > 0
            ? '...'
            : ''
          : `\n... ${group.start - previous.end} chars omitted ...\n`;
      const suffix =
        index === groups.length - 1 && group.end < prompt.length ? '...' : '';

      return `${omitted}${highlightExcerpt(prompt, group)}${suffix}`;
    })
    .join('');

const highlightExcerpt = (prompt, { start, end, match }) => {
  const highlightStart = clamp(match.start, start, end);
  const highlightEnd = clamp(match.end, start, end);

  return [
    prompt.slice(start, highlightStart),
    markAmbiguousText(prompt.slice(highlightStart, highlightEnd)),
    prompt.slice(highlightEnd, end),
  ].join('');
};

const markAmbiguousText = (text) => `[[${ANSI_INVERSE}${text}${ANSI_RESET}]]`;

const clamp = (value, min, max) => Math.min(Math.max(value, min), max);

const rewritePrompt = async (credential, prompt, risk, decision, iteration) => {
  const input = `
    # Task
    Rewrite the current prompt in memory by applying the selected ambiguity resolution.
    Preserve all unrelated wording, ordering, and requirements.
    If the selected resolution came from free-form user text, treat that text as the user's intended resolution.
    Return the full rewritten prompt string, not a patch.

    # Current prompt
    <current_prompt>
    ${prompt}
    </current_prompt>

    # Ambiguity risk
    ${formatRiskForPrompt(risk)}

    # Selected resolution
    Label: ${decision.label}
    Resolution: ${decision.resolution}
    Prompt change: ${decision.promptChange}
  `;

  console.log(`[loop:${iteration}] rewrite call`, {
    risk: risk.risk,
    selectedOption: decision,
  });

  const response = await createRewriteAgent(credential).complete(input, {
    schema: rewriteSchema,
  });
  const result = response.structured;

  if (result.prompt.trim() === '') {
    throw new Error('Rewrite agent returned an empty prompt.');
  }

  console.log(`[loop:${iteration}] rewrite result`, {
    finishReason: response.finishReason,
    changeSummary: result.changeSummary,
    promptChars: result.prompt.length,
    prompt: result.prompt,
  });

  return result;
};

const formatRiskForPrompt = ({ risk, kind, reason, text, line }) => `
  Kind: ${kind}
  Risk: ${risk}
  Reason: ${reason}
  Text: ${text}
  Line: ${line}
`;

const createResolvedAmbiguityHistoryRecord = (
  iteration,
  risk,
  decision,
  rewrite,
) => ({
  iteration,
  risk: {
    kind: risk.kind,
    risk: risk.risk,
    reason: risk.reason,
    text: risk.text,
    line: risk.line,
  },
  selected: {
    label: decision.label,
    resolution: decision.resolution,
    promptChange: decision.promptChange,
  },
  applied: {
    changeSummary: rewrite.changeSummary,
    promptChars: rewrite.prompt.length,
  },
});

const truncateChoiceLabel = (value) => {
  const singleLine = value.replace(/\s+/g, ' ').trim();
  return singleLine.length <= 120
    ? singleLine
    : `${singleLine.slice(0, 117)}...`;
};

const formatTextQuestion = ({ question, guidance, placeholder }) =>
  [question, guidance, formatPlaceholder(placeholder)]
    .filter((part) => part.trim() !== '')
    .join('\n');

const formatPlaceholder = (placeholder) =>
  placeholder.trim() === '' ? '' : `Suggested answer format: ${placeholder}`;

const errorMessage = (error) =>
  error instanceof Error ? error.message : String(error);

const main = async () => {
  console.log('[startup] reading environment and source prompt', {
    promptChars: TARGET_PROMPT.length,
  });

  const env = parseEnv(await readEnvFile());
  const credential = codexCredentialFromToken(
    required(env, CODEX_AUTHORIZATION_ENV_KEY),
  );

  let currentPrompt = TARGET_PROMPT;
  let iteration = 1;
  let resolvedAmbiguityHistory = [];

  while (true) {
    console.log(`[loop:${iteration}] iteration started`, {
      promptChars: currentPrompt.length,
    });

    const risk = await detectRisk(
      credential,
      currentPrompt,
      iteration,
      resolvedAmbiguityHistory,
    );

    if (risk === undefined) {
      console.log(`[loop:${iteration}] stop condition`, {
        reason: 'No ambiguity risks remain.',
      });
      break;
    }

    const options = await extractOptions(
      credential,
      currentPrompt,
      risk,
      iteration,
    );
    const decision = await chooseOption(
      currentPrompt,
      risk,
      options,
      iteration,
    );
    const rewrite = await rewritePrompt(
      credential,
      currentPrompt,
      risk,
      decision,
      iteration,
    );
    resolvedAmbiguityHistory = [
      ...resolvedAmbiguityHistory,
      createResolvedAmbiguityHistoryRecord(iteration, risk, decision, rewrite),
    ];
    currentPrompt = rewrite.prompt;
    console.log(`[loop:${iteration}] resolved ambiguity recorded`, {
      resolvedAmbiguityHistoryCount: resolvedAmbiguityHistory.length,
    });
    iteration += 1;
  }

  console.log('[final] final prompt');
  console.log(currentPrompt);
};

main().catch((error) => {
  console.error(errorMessage(error));
  process.exitCode = 1;
});
