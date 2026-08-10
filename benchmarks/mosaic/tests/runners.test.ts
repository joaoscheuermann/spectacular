import assert from 'node:assert/strict';
import { mkdtemp, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import type { Agent } from 'agent';
import type { Skill } from 'bundle';
import type { MosaicAgent, MosaicEvent, MosaicOptions } from 'mosaic';
import type { Tool } from 'tool';
import { z } from 'zod';

import { direct } from '../src/direct.js';
import { mosaic } from '../src/mosaic.js';
import { createProvider, defaultModel, type RunEvent } from '../src/run.js';
import { fakeProvider, finish } from './support/provider.js';

const skill: Skill = {
  name: 'inspect',
  description: 'Inspect the workspace.',
  body: 'Read relevant files before changing them.',
  allowedTools: ['terminal'],
  indexText: 'inspect',
};

test('provider composition uses Unified OpenRouter with the BenchFlow proxy', async () => {
  let authorization: string | undefined;
  let url: string | undefined;
  const environment = {
    OPENROUTER_BASE_URL: 'https://proxy.invalid/v1',
    OPENROUTER_API_KEY: 'secret',
    OPENROUTER_MODEL: 'proxy/model',
  };
  const profile = createProvider({
    environment,
    transport: {
      request: async (request) => {
        authorization = request.headers?.authorization;
        url = request.url;
        return { status: 200, headers: {}, body: '{"data":[]}' };
      },
      stream: async function* () {},
    },
  });

  await profile.provider.models();

  assert.equal(profile.provider.metadata.id, 'unified');
  assert.equal(profile.model, 'proxy/model');
  assert.equal(defaultModel, 'openai/gpt-5.6-luna');
  assert.equal(authorization, 'Bearer secret');
  assert.equal(url, 'https://proxy.invalid/v1/models');
  assert.equal(environment.OPENROUTER_API_KEY, 'secret');
});

test('provider composition defaults to the OpenRouter endpoint', async () => {
  let url: string | undefined;
  const profile = createProvider({
    environment: { OPENROUTER_API_KEY: 'secret' },
    transport: {
      request: async (request) => {
        url = request.url;
        return { status: 200, headers: {}, body: '{"data":[]}' };
      },
      stream: async function* () {},
    },
  });

  await profile.provider.models();

  assert.equal(url, 'https://openrouter.ai/api/v1/models');
});

test('reads, deletes, and caches the process credential file', async () => {
  const root = await mkdtemp(join(tmpdir(), 'mosaic-provider-'));
  const path = join(root, 'credential');
  const sensitive = Object.entries(process.env).filter(([name]) =>
    /(?:master|private|api|access)[_-]?key|auth(?:orization)?|bearer|token|secret|password|credentials?|cookie/iu.test(
      name,
    ),
  );
  const baseUrl = process.env.OPENROUTER_BASE_URL;
  const model = process.env.OPENROUTER_MODEL;
  const authorizations: string[] = [];

  try {
    await writeFile(path, 'file-secret', { mode: 0o600 });
    process.env.OPENROUTER_API_KEY_FILE = path;
    process.env.OPENROUTER_API_KEY = 'direct-secret';
    process.env.BENCHFLOW_PROVIDER_API_KEY = 'benchflow-secret';
    process.env.BENCHFLOW_LITELLM_MASTER_KEY = 'master-secret';
    process.env.OPENROUTER_BASE_URL = 'https://proxy.invalid/v1';
    process.env.OPENROUTER_MODEL = 'proxy/model';
    const profile = createProvider({
      transport: {
        request: async (request) => {
          authorizations.push(request.headers?.authorization ?? '');
          return { status: 200, headers: {}, body: '{"data":[]}' };
        },
        stream: async function* () {},
      },
    });

    await profile.provider.models();
    await profile.provider.models();

    assert.deepEqual(authorizations, [
      'Bearer file-secret',
      'Bearer file-secret',
    ]);
    await assert.rejects(stat(path));
    assert.equal(process.env.OPENROUTER_API_KEY, undefined);
    assert.equal(process.env.BENCHFLOW_PROVIDER_API_KEY, undefined);
    assert.equal(process.env.BENCHFLOW_LITELLM_MASTER_KEY, undefined);
    assert.equal(process.env.OPENROUTER_API_KEY_FILE, undefined);
  } finally {
    for (const name of Object.keys(process.env)) {
      if (
        /(?:master|private|api|access)[_-]?key|auth(?:orization)?|bearer|token|secret|password|credentials?|cookie/iu.test(
          name,
        )
      ) {
        delete process.env[name];
      }
    }
    for (const [name, value] of sensitive) process.env[name] = value;
    if (baseUrl === undefined) delete process.env.OPENROUTER_BASE_URL;
    else process.env.OPENROUTER_BASE_URL = baseUrl;
    if (model === undefined) delete process.env.OPENROUTER_MODEL;
    else process.env.OPENROUTER_MODEL = model;
    await rm(root, { recursive: true, force: true });
  }
});

test('rejects a direct process credential after scrubbing it', () => {
  const sensitive = Object.entries(process.env).filter(([name]) =>
    /(?:master|private|api|access)[_-]?key|auth(?:orization)?|bearer|token|secret|password|credentials?|cookie/iu.test(
      name,
    ),
  );

  try {
    process.env.OPENROUTER_API_KEY = 'direct-secret';
    process.env.BENCHFLOW_LITELLM_MASTER_KEY = 'master-secret';
    delete process.env.OPENROUTER_API_KEY_FILE;

    assert.throws(
      () => createProvider(),
      /OPENROUTER_API_KEY_FILE is required/u,
    );
    assert.equal(process.env.OPENROUTER_API_KEY, undefined);
    assert.equal(process.env.BENCHFLOW_LITELLM_MASTER_KEY, undefined);
  } finally {
    for (const name of Object.keys(process.env)) {
      if (
        /(?:master|private|api|access)[_-]?key|auth(?:orization)?|bearer|token|secret|password|credentials?|cookie/iu.test(
          name,
        )
      ) {
        delete process.env[name];
      }
    }
    for (const [name, value] of sensitive) process.env[name] = value;
  }
});

test('runners capture their default provider profile at construction', async () => {
  const previous = process.env.OPENROUTER_MODEL;
  const root = await mkdtemp(join(tmpdir(), 'mosaic-runner-profile-'));
  const directModels: string[] = [];
  const mosaicModels: string[] = [];

  try {
    process.env.OPENROUTER_MODEL = 'model-at-construction';
    const directCredential = join(root, 'direct-credential');
    await writeFile(directCredential, 'file-secret', { mode: 0o600 });
    process.env.OPENROUTER_API_KEY_FILE = directCredential;
    const directRunner = direct({
      loadSkills: async () => [],
      createTerminal: () => terminal,
      createAgent: (options) => {
        directModels.push(options.model);
        return idleAgent;
      },
    });
    const mosaicCredential = join(root, 'mosaic-credential');
    await writeFile(mosaicCredential, 'file-secret', { mode: 0o600 });
    process.env.OPENROUTER_API_KEY_FILE = mosaicCredential;
    const mosaicRunner = mosaic({
      loadSkills: async () => [],
      createTerminal: () => terminal,
      createWorkflow: (options) => {
        mosaicModels.push(options.models.execution.model);
        return blockedWorkflow;
      },
    });

    process.env.OPENROUTER_MODEL = 'model-at-prompt';
    await directRunner.run({ prompt: 'one', cwd: '/work' }, () => undefined);
    await directRunner.run({ prompt: 'two', cwd: '/work' }, () => undefined);
    await mosaicRunner.run({ prompt: 'one', cwd: '/work' }, () => undefined);
    await mosaicRunner.run({ prompt: 'two', cwd: '/work' }, () => undefined);

    assert.deepEqual(directModels, [
      'model-at-construction',
      'model-at-construction',
    ]);
    assert.deepEqual(mosaicModels, [
      'model-at-construction',
      'model-at-construction',
    ]);
  } finally {
    if (previous === undefined) delete process.env.OPENROUTER_MODEL;
    else process.env.OPENROUTER_MODEL = previous;
    delete process.env.OPENROUTER_API_KEY_FILE;
    await rm(root, { recursive: true, force: true });
  }
});

test('direct runner creates isolated agents with one terminal and all skills', async () => {
  const fake = fakeProvider((request) =>
    request.messages.some(({ role }) => role === 'tool')
      ? finish('done')
      : finish('', [
          {
            id: 'call-1',
            name: 'terminal',
            arguments: JSON.stringify({ command: 'pwd' }),
          },
        ]),
  );
  const calls: { cwd: string; signal?: AbortSignal }[] = [];
  const runner = direct({
    profile: { provider: fake.provider, model: 'same-model' },
    home: '/benchmark-home',
    loadSkills: async (home) => {
      assert.equal(home, '/benchmark-home');
      return [skill];
    },
    createTerminal: (options) => {
      calls.push(options);
      return terminal;
    },
  });
  const events: RunEvent[] = [];

  await runner.run({ prompt: 'first', cwd: '/work' }, (event) => {
    events.push(event);
  });
  await runner.run({ prompt: 'second', cwd: '/work' }, () => undefined);

  assert.equal(calls.length, 2);
  assert.equal(fake.requests[0]?.model, 'same-model');
  assert.equal(fake.requests[0]?.effort, 'low');
  assert.deepEqual(
    fake.requests[0]?.tools?.map(({ name }) => name),
    ['terminal'],
  );
  assert.match(
    String(fake.requests[0]?.messages[0]?.content),
    /Skill: inspect/u,
  );
  assert.equal(
    fake.requests.filter(
      ({ messages }) =>
        messages.filter(({ role }) => role === 'user').length === 1,
    ).length,
    4,
  );
  assert.equal(
    events.some(
      (event) =>
        event.type === 'message_delta' && event.delta === 'private reasoning',
    ),
    false,
  );
  assert.equal(
    events.some(({ type }) => type === 'tool_started'),
    true,
  );
  assert.equal(
    events.some(({ type }) => type === 'tool_completed'),
    true,
  );
});

test('mosaic runner keeps provider model skills and terminal in parity', async () => {
  const fake = fakeProvider();
  const signal = new AbortController().signal;
  let configured: MosaicOptions | undefined;
  let promptOptions: Parameters<MosaicAgent['prompt']>[1];
  const emitted: RunEvent[] = [];
  const createWorkflow = (options: MosaicOptions): MosaicAgent => {
    configured = options;
    return {
      prompt: async (_prompt, options) => {
        assert.ok(options !== undefined);
        promptOptions = options;
        await options.observer?.(
          hook({
            type: 'tool.started',
            stage: 'execution',
            nodeId: 'node-1',
            revision: 1,
            callId: 'call-1',
            toolName: 'terminal',
            input: { command: 'pwd' },
          }),
        );
        return {
          status: 'completed',
          delivery: { markdown: 'Final answer.', parts: [] },
          nodes: [],
        };
      },
    };
  };
  const runner = mosaic({
    profile: { provider: fake.provider, model: 'same-model' },
    home: '/benchmark-home',
    loadSkills: async () => [skill],
    createTerminal: () => terminal,
    createWorkflow,
    randomUUID: () => '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601',
  });

  await runner.run({ prompt: 'request', cwd: '/work', signal }, (event) => {
    emitted.push(event);
  });

  assert.ok(configured !== undefined);
  assert.equal(configured.providers.planning, fake.provider);
  assert.equal(configured.providers.revision, fake.provider);
  assert.equal(configured.providers.execution, fake.provider);
  assert.equal(configured.providers.reranker, fake.provider);
  assert.deepEqual(configured.models, {
    planning: { model: 'same-model', effort: 'low' },
    revision: { model: 'same-model', effort: 'low' },
    execution: { model: 'same-model', effort: 'low' },
    reranker: 'same-model',
    embedder: 'same-model',
  });
  assert.deepEqual(configured.routing, {
    maxHintCandidates: 1,
    maxRetrievedCandidates: 1,
    maxSkills: 0,
  });
  assert.deepEqual(configured.revision, { max: 3 });
  assert.deepEqual(configured.execution, { maxTurns: 32 });
  assert.deepEqual(configured.skills.required, [skill]);
  assert.equal(configured.skills.required, configured.skills.menu);
  assert.deepEqual(configured.tools.required, [terminal]);
  assert.equal(configured.tools.required[0], configured.tools.menu[0]);
  assert.deepEqual(await configured.skills.retriever.search('query', 1), []);
  assert.equal(promptOptions?.capture, 'io');
  assert.equal(promptOptions?.signal, signal);
  assert.equal(promptOptions?.runId, '018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601');
  assert.equal(
    emitted.some(({ type }) => type === 'tool_started'),
    true,
  );
  assert.equal(
    emitted.some(
      (event) =>
        event.type === 'message_delta' && event.delta === 'Final answer.',
    ),
    true,
  );
});

const input = z.object({ command: z.string() });
const output = z.object({ exit_code: z.number() });
const terminal: Tool<typeof input, typeof output> = {
  name: 'terminal',
  description: 'Run a command.',
  input,
  output,
  definition: {
    name: 'terminal',
    description: 'Run a command.',
    inputSchema: {
      type: 'object',
      properties: { command: { type: 'string' } },
      required: ['command'],
      additionalProperties: false,
    },
    outputSchema: {
      type: 'object',
      properties: { exit_code: { type: 'number' } },
      required: ['exit_code'],
      additionalProperties: false,
    },
    strict: true,
  },
  execute: async () => ({ exit_code: 0 }),
};

const hook = (value: object): MosaicEvent =>
  ({ schemaVersion: 2, runId: 'run-1', sequence: 1, ...value }) as MosaicEvent;

const idleAgent = {
  complete: async () => {
    throw new Error('Complete is not used by this test.');
  },
  stream: async function* () {},
} as Agent;

const blockedWorkflow: MosaicAgent = {
  prompt: async () => ({ status: 'blocked', nodes: [] }),
};
