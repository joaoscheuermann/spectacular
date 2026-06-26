#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import { lstat, open, readdir, readFile } from 'node:fs/promises';
import { dirname, isAbsolute, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

import { createAgent } from 'agent';
import { createCodexProvider, createFetchTransport } from 'llms';
import { createMessageStorage } from 'messages';
import { codexCredentialFromToken } from 'oauth';
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

const SYSTEM_PROMPT = `
# Goal
Identify all "ambiguity risks" in the provided text that could lead to incorrect understanding, false assumptions, or operational mistakes. Analyze and flag these risks in order of priority, focusing first on foundational issues (like pronoun reference) before addressing broader contextual issues.

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
`;

const TARGET_PROMPT = readFileSync(
  `C:\\Users\\jvito\\Documents\\git\\spectacular\\doric\\docs\\00-references\\01-source-prompt.md`,
  {
    encoding: 'utf-8',
  },
);

const schema = z.object({
  risks: z.array(
    z.object({
      risk: z.string(),
      kind: z.enum(['semantic', 'syntactic', 'pragmatic', 'referential']),
      reason: z.string(),
      texts: z.array(
        z.object({
          text: z.string(),
          startCharIndex: z.number(),
          endCharIndex: z.number(),
        }),
      ),
    }),
  ),
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
    execute: ({ path = '.', limit = MAX_LIST_ENTRIES }) =>
      listCodebaseFiles(path, limit),
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
    execute: ({ path, maxBytes = MAX_READ_BYTES }) =>
      readCodebaseFile(path, maxBytes),
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
    execute: ({
      path = '.',
      query,
      ignoreCase = false,
      limit = MAX_SEARCH_MATCHES,
    }) => searchCodebaseText(path, query, ignoreCase, limit),
  });

const createCodebaseTools = () => [
  listCodebaseFilesTool(),
  readCodebaseFileTool(),
  searchCodebaseTextTool(),
];

const listCodebaseFiles = async (path, limit) => {
  console.log(`Calling listCodebaseFiles`);

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
  console.log(`Calling readCodebaseFile`);

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
  console.log(`Calling searchCodebaseText`);

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

const createRiskAgent = (credential) =>
  createAgent({
    provider: createCodexProvider({
      transport: createFetchTransport(),
      authorization: credential.authorization,
      chatGptAccountId: credential.accountId,
      fedramp: credential.fedramp,
    }),
    tools: createToolStorage([]),
    messages: createMessageStorage(),
    system: SYSTEM_PROMPT,
    model: 'gpt-5.5',
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
      Given the prompt and the issue provided, write a question or a set of questions with options that might help solve the issue.

      Explore a given codebase when appropriate to acquire context.

      You have access to the following tools:
      ${tools
        .map(
          (tool) => `${tool.name}: ${tool.description}, args: ${tool.schema}`,
        )
        .join('\n')}
    `,
    model: 'gpt-5.5',
  });
};
const main = async () => {
  const env = parseEnv(await readEnvFile());
  const credential = codexCredentialFromToken(
    required(env, CODEX_AUTHORIZATION_ENV_KEY),
  );

  const risksAgent = createRiskAgent(credential);
  const solutionsAgent = createSolutionsAgent(credential);

  console.log('Started');

  const response = await risksAgent.complete(TARGET_PROMPT, { schema });

  const { risks } = response.structured;

  console.log(risks);

  for (const { risk, kind, reason, texts } of risks) {
    console.log(`Working on:`, risk);

    const PROMPT = `
      # Initial prompt
      ~~~${TARGET_PROMPT}~~~

      # Risk
      This is a ${kind} ambiguity, The ambiguity is: ${risk} because ${reason}

      ## Texts where it happens
      ${texts
        .map(
          ({ text, startCharIndex, endCharIndex }) =>
            `${text}, start: ${startCharIndex + 1} end: ${endCharIndex + 1}`,
        )
        .join('\n')}
    `;

    const schema = z.object({
      questions: z.array(
        z.object({
          question: z.string(),
          options: z.array(
            z.object({
              text: z.string(),
              value: z.number(),
            }),
          ),
        }),
      ),
    });

    const response = await solutionsAgent.complete(PROMPT, {
      schema,
    });

    const { questions } = response.structured;

    for (const question of questions) console.log(question);
  }
};

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
