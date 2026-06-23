import assert from 'node:assert/strict';
import test from 'node:test';

import { A2AError, type AgentExecutionEvent } from '@a2a-js/sdk/server';
import { createSessionStore } from 'session';
import type { PromptArtifact } from 'workflow-prompt';

import type { DoricSessionContext } from '../src/lib/executor.js';
import {
  createConfig,
  createDoricTestHarness,
  createEventBus,
  createRequestContext,
  createTextPart,
} from './fakes.js';

test('initializes a sandbox session and clones the configured repo on the first message', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });
  const eventBus = createEventBus();
  const requestContext = createRequestContext({
    contextId: 'context-1',
  });

  await harness.executor.execute(requestContext, eventBus);

  assert.equal(harness.dockerCreateCount(), 1);
  assert.deepEqual(harness.sandboxOptions[0], {
    docker: harness.docker,
    image: 'node:slim',
    name: 'doric-context-1',
    network: { mode: 'bridge' },
  });
  assert.deepEqual(harness.sandboxes[0]?.clones, [
    {
      url: 'https://github.com/example/repo',
      auth: { kind: 'token', token: 'github-token' },
    },
  ]);
  assert.deepEqual(sessions.get('context-1')?.repo, {
    path: '/workspace/repo',
    commit: 'abc123',
  });
  assert.deepEqual(
    eventBus.events.map((event) => event.kind),
    [
      'task',
      'status-update',
      'status-update',
      'status-update',
      'status-update',
      'status-update',
      'status-update',
      'artifact-update',
      'status-update',
    ],
  );
  assert.deepEqual(eventBus.events.flatMap(statusTextParts), [
    { kind: 'text', text: `Created task ${requestContext.taskId}.` },
    { kind: 'text', text: 'Checking for an existing session.' },
    { kind: 'text', text: 'Creating a sandbox for the repository.' },
    { kind: 'text', text: 'Checking Git inside the sandbox.' },
    { kind: 'text', text: 'Cloning the configured repository.' },
    { kind: 'text', text: 'Repository session is ready.' },
    { kind: 'text', text: 'Running workflow-prompt.' },
    {
      kind: 'text',
      text: 'Prompt workflow completed. PROMPT artifact is ready.',
    },
  ]);
  assert.equal(eventBus.finishedCount, 1);
});

const statusTextParts = (
  event: AgentExecutionEvent,
): { kind: 'text'; text: string }[] => {
  if (event.kind !== 'task' && event.kind !== 'status-update') {
    return [];
  }

  const part = event.status.message?.parts[0];

  return part?.kind === 'text' ? [part] : [];
};

const artifactText = (event: AgentExecutionEvent): string | undefined => {
  if (event.kind !== 'artifact-update') {
    return undefined;
  }

  const part = event.artifact.parts[0];

  return part?.kind === 'text' ? part.text : undefined;
};

const completedPromptArtifact = (artifact: PromptArtifact): PromptArtifact => ({
  ...artifact,
  data: {
    ...artifact.data,
    requestUnderstanding: 'The user wants the agent built.',
    intent: {
      goal: 'Build the agent',
      scope: 'TypeScript agent core',
    },
    productRequirements: ['Run the prompt workflow.'],
    technicalRequirements: ['Publish A2A events in order.'],
  },
});

const promptArtifactWithQuestions = (
  artifact: PromptArtifact,
): PromptArtifact => ({
  ...artifact,
  data: {
    ...artifact.data,
    openQuestions: [
      {
        question: 'Which provider should handle coding tasks?',
        impact: 'The workflow cannot choose a final model path.',
        recommendation: 'Use the configured coding task model.',
      },
    ],
  },
});

test('runs workflow-prompt after the repository session is ready', async () => {
  const harness = createDoricTestHarness({
    promptRunner: async (artifact) => completedPromptArtifact(artifact),
  });
  const eventBus = createEventBus();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      parts: [createTextPart('Design the prompt handoff')],
    }),
    eventBus,
  );

  assert.equal(harness.promptRuns.length, 1);
  assert.equal(
    harness.promptRuns[0]?.artifact.data.initialUserPrompt,
    'Design the prompt handoff',
  );
  assert.equal(harness.promptRuns[0]?.options.workspaceRoot, '/workspace/repo');
  assert.equal(harness.promptRuns[0]?.options.model, 'gpt-5');
  assert.equal(harness.providerConfigs[0]?.id, 'openai');
  assert.ok(
    eventBus.events.findIndex(
      (event) =>
        event.kind === 'status-update' &&
        event.status.message?.parts[0]?.kind === 'text' &&
        event.status.message.parts[0].text === 'Repository session is ready.',
    ) <
      eventBus.events.findIndex(
        (event) =>
          event.kind === 'status-update' &&
          event.status.message?.parts[0]?.kind === 'text' &&
          event.status.message.parts[0].text === 'Running workflow-prompt.',
      ),
  );
});

test('uses the coding task model when another task is configured first', async () => {
  const harness = createDoricTestHarness({
    promptRunner: async (artifact) => completedPromptArtifact(artifact),
  });

  await harness.executor.execute(
    createRequestContext({
      config: createConfig({
        providers: [
          { id: 'openai', type: 'openai', token: 'provider-token' },
          { id: 'openrouter', type: 'openrouter', token: 'router-token' },
        ],
        models: [
          { id: 'planning-model', provider: 'openai', model: 'gpt-5-mini' },
          {
            id: 'coding-model',
            provider: 'openrouter',
            model: 'anthropic/claude-sonnet-4',
          },
        ],
        tasks: [
          { id: 'planning', model: 'planning-model' },
          { id: 'coding', model: 'coding-model' },
        ],
      }),
    }),
    createEventBus(),
  );

  assert.equal(
    harness.promptRuns[0]?.options.model,
    'anthropic/claude-sonnet-4',
  );
  assert.equal(harness.providerConfigs[0]?.id, 'openrouter');
});

test('uses configured OpenAI bearer authorization without prefixing it again', async () => {
  const originalFetch = globalThis.fetch;
  const requests: RequestInit[] = [];
  globalThis.fetch = (async (_input, init) => {
    requests.push(init ?? {});

    return new Response(
      JSON.stringify({ status: 'completed', output_text: 'ok', output: [] }),
      { status: 200, headers: { 'content-type': 'application/json' } },
    );
  }) as typeof fetch;

  try {
    const harness = createDoricTestHarness({
      useDefaultProvider: true,
      promptRunner: async (artifact, options) => {
        await options.provider.complete({
          model: options.model,
          messages: [{ role: 'user', content: 'Hi' }],
        });

        return completedPromptArtifact(artifact);
      },
    });

    await harness.executor.execute(
      createRequestContext({
        config: createConfig({
          providers: [
            {
              id: 'openai',
              type: 'openai',
              token: 'Bearer session-token',
            },
          ],
        }),
      }),
      createEventBus(),
    );

    const headers = requests[0]?.headers as Record<string, string> | undefined;

    assert.equal(headers?.['authorization'], 'Bearer session-token');
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test('uses Codex ChatGPT auth headers for Codex providers', async () => {
  const originalFetch = globalThis.fetch;
  const requests: RequestInit[] = [];
  const accessToken = jwt({
    exp: Math.floor(Date.now() / 1000) + 3600,
    'https://api.openai.com/auth': {
      chatgpt_account_id: 'acct_123',
    },
  });

  globalThis.fetch = (async (_input, init) => {
    requests.push(init ?? {});

    return new Response(
      JSON.stringify({ status: 'completed', output_text: 'ok', output: [] }),
      { status: 200, headers: { 'content-type': 'application/json' } },
    );
  }) as typeof fetch;

  try {
    const harness = createDoricTestHarness({
      useDefaultProvider: true,
      promptRunner: async (artifact, options) => {
        await options.provider.complete({
          model: options.model,
          messages: [{ role: 'user', content: 'Hi' }],
        });

        return completedPromptArtifact(artifact);
      },
    });

    await harness.executor.execute(
      createRequestContext({
        config: createConfig({
          providers: [
            {
              id: 'codex',
              type: 'codex',
              token: `Bearer ${accessToken}`,
            },
          ],
          models: [{ id: 'default', provider: 'codex', model: 'gpt-5' }],
        }),
      }),
      createEventBus(),
    );

    const headers = requests[0]?.headers as Record<string, string> | undefined;

    assert.equal(headers?.authorization, `Bearer ${accessToken}`);
    assert.equal(headers?.['ChatGPT-Account-ID'], 'acct_123');
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test('publishes open questions before ending the turn as input-required', async () => {
  const harness = createDoricTestHarness({
    promptRunner: async (artifact) => promptArtifactWithQuestions(artifact),
  });
  const eventBus = createEventBus();

  await harness.executor.execute(createRequestContext(), eventBus);

  const artifactIndex = eventBus.events.findIndex(
    (event) => event.kind === 'artifact-update',
  );
  const finalIndex = eventBus.events.findIndex(
    (event) => event.kind === 'status-update' && event.final,
  );
  const finalEvent = eventBus.events[finalIndex];

  assert.ok(artifactIndex >= 0);
  assert.equal(finalIndex, eventBus.events.length - 1);
  assert.ok(artifactIndex < finalIndex);
  assert.equal(finalEvent?.kind, 'status-update');
  assert.equal(finalEvent.status.state, 'input-required');
  assert.match(
    finalEvent.status.message?.parts[0]?.kind === 'text'
      ? finalEvent.status.message.parts[0].text
      : '',
    /Which provider should handle coding tasks\?/u,
  );
  assert.match(
    finalEvent.status.message?.parts[0]?.kind === 'text'
      ? finalEvent.status.message.parts[0].text
      : '',
    /Recommendation: Use the configured coding task model\./u,
  );
  assert.ok(
    eventBus.events.every(
      (event) =>
        event.kind !== 'status-update' || event.status.state !== 'completed',
    ),
  );
});

test('publishes the prompt artifact before the completed final status', async () => {
  const harness = createDoricTestHarness({
    promptRunner: async (artifact) => completedPromptArtifact(artifact),
  });
  const eventBus = createEventBus();

  await harness.executor.execute(
    createRequestContext({ parts: [createTextPart('Finish the prompt')] }),
    eventBus,
  );

  const artifactIndex = eventBus.events.findIndex(
    (event) => event.kind === 'artifact-update',
  );
  const finalIndex = eventBus.events.findIndex(
    (event) => event.kind === 'status-update' && event.final,
  );
  const finalEvent = eventBus.events[finalIndex];

  assert.ok(artifactIndex >= 0);
  assert.equal(finalIndex, eventBus.events.length - 1);
  assert.ok(artifactIndex < finalIndex);
  assert.match(artifactText(eventBus.events[artifactIndex]) ?? '', /# Prompt/u);
  assert.equal(finalEvent?.kind, 'status-update');
  assert.equal(finalEvent.status.state, 'completed');
  assert.equal(
    finalEvent.status.message?.parts[0]?.kind === 'text'
      ? finalEvent.status.message.parts[0].text
      : '',
    'Prompt workflow completed. PROMPT artifact is ready.',
  );
});

test('normalizes context IDs before using them as Docker container names', async () => {
  const harness = createDoricTestHarness();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'workspace:feature/one',
    }),
    createEventBus(),
  );

  assert.equal(harness.sandboxOptions[0]?.name, 'doric-workspace-feature-one');
});

test('reuses an existing context without requiring config on later messages', async () => {
  const harness = createDoricTestHarness();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
    }),
    createEventBus(),
  );
  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      metadata: null,
      parts: [createTextPart('continue')],
    }),
    createEventBus(),
  );

  assert.equal(harness.dockerCreateCount(), 1);
  assert.equal(harness.sandboxes.length, 1);
  assert.equal(harness.sandboxes[0]?.clones.length, 1);
});

test('isolates sessions across context IDs', async () => {
  const harness = createDoricTestHarness();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-a',
      config: createConfig({
        repoUrl: 'https://github.com/example/repo-a',
        token: 'token-a',
      }),
    }),
    createEventBus(),
  );
  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-b',
      config: createConfig({
        repoUrl: 'https://github.com/example/repo-b',
        token: 'token-b',
      }),
    }),
    createEventBus(),
  );

  assert.equal(harness.dockerCreateCount(), 2);
  assert.deepEqual(
    harness.sandboxes.map((sandbox) => sandbox.clones[0]?.url),
    ['https://github.com/example/repo-a', 'https://github.com/example/repo-b'],
  );
});

test('rejects missing config when the context is unknown', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });

  await assert.rejects(
    harness.executor.execute(
      createRequestContext({
        contextId: 'context-1',
        metadata: null,
        parts: [createTextPart('missing config')],
      }),
      createEventBus(),
    ),
    (error: unknown) => {
      assert.ok(error instanceof A2AError);
      assert.equal(error.code, -32602);
      assert.deepEqual(error.data, {
        code: 'missing_configuration',
        path: 'message.metadata.configuration',
      });

      return true;
    },
  );
  assert.deepEqual(sessions.list(), []);
});

test('rejects empty user prompt text before session setup', async () => {
  const harness = createDoricTestHarness();

  await assert.rejects(
    harness.executor.execute(
      createRequestContext({
        parts: [createTextPart('   ')],
      }),
      createEventBus(),
    ),
    (error: unknown) => {
      assert.ok(error instanceof A2AError);
      assert.equal(error.code, -32602);
      assert.deepEqual(error.data, {
        code: 'missing_prompt_text',
        path: 'message.parts',
      });

      return true;
    },
  );
  assert.equal(harness.dockerCreateCount(), 0);
  assert.equal(harness.promptRuns.length, 0);
});

test('does not require config for an in-flight context', async () => {
  const harness = createDoricTestHarness();
  const first = harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
    }),
    createEventBus(),
  );
  const second = harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      metadata: null,
      parts: [createTextPart('continue')],
    }),
    createEventBus(),
  );

  await Promise.all([first, second]);

  assert.equal(harness.dockerCreateCount(), 1);
});

test('updates only stored config when later message includes configuration', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });
  const initialConfig = createConfig({
    repoUrl: 'https://github.com/example/initial',
    token: 'initial-token',
  });
  const updatedConfig = createConfig({
    repoUrl: 'https://github.com/example/updated',
    token: 'updated-token',
  });

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      config: initialConfig,
    }),
    createEventBus(),
  );
  const session = sessions.get('context-1');

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      config: updatedConfig,
      parts: [createTextPart('continue')],
    }),
    createEventBus(),
  );

  assert.equal(harness.dockerCreateCount(), 1);
  assert.equal(sessions.get('context-1'), session);
  assert.deepEqual(sessions.get('context-1')?.config, updatedConfig);
  assert.deepEqual(harness.sandboxes[0]?.clones, [
    {
      url: 'https://github.com/example/initial',
      auth: { kind: 'token', token: 'initial-token' },
    },
  ]);
  assert.deepEqual(sessions.get('context-1')?.repo, {
    path: '/workspace/repo',
    commit: 'abc123',
  });
});

test('rejects invalid later config and leaves stored config unchanged', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });
  const initialConfig = createConfig();

  await harness.executor.execute(
    createRequestContext({
      contextId: 'context-1',
      config: initialConfig,
    }),
    createEventBus(),
  );

  await assert.rejects(
    harness.executor.execute(
      createRequestContext({
        contextId: 'context-1',
        metadata: {
          configuration: {
            ...initialConfig,
            models: [{ id: 'planning', provider: 'openai', model: 1 }],
          },
        },
        parts: [createTextPart('continue')],
      }),
      createEventBus(),
    ),
    (error: unknown) => {
      assert.ok(error instanceof A2AError);
      assert.equal(error.code, -32602);
      assert.deepEqual(error.data, {
        code: 'invalid_config_field',
        path: 'message.metadata.configuration.models[0].model',
      });

      return true;
    },
  );

  assert.deepEqual(sessions.get('context-1')?.config, initialConfig);
  assert.equal(harness.dockerCreateCount(), 1);
  assert.equal(harness.sandboxes[0]?.clones.length, 1);
});

test('uses SDK-generated context IDs as session keys', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({ sessions });
  const requestContext = createRequestContext({});

  await harness.executor.execute(requestContext, createEventBus());

  assert.ok(sessions.get(requestContext.contextId));
  assert.equal(harness.sandboxes[0]?.clones.length, 1);
});

test('skips Git installation when Git is already available', async () => {
  const harness = createDoricTestHarness({ hasGit: true });

  await harness.executor.execute(createRequestContext(), createEventBus());

  assert.deepEqual(
    harness.sandboxes[0]?.execs.map((input) => input.cmd.join(' ')),
    ['sh -lc command -v git >/dev/null 2>&1'],
  );
});

test('installs Git and CA certificates when Git is missing', async () => {
  const harness = createDoricTestHarness({ hasGit: false });

  await harness.executor.execute(createRequestContext(), createEventBus());

  assert.deepEqual(
    harness.sandboxes[0]?.execs.map((input) => input.cmd.join(' ')),
    [
      'sh -lc command -v git >/dev/null 2>&1',
      'sh -lc apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*',
      'sh -lc command -v git >/dev/null 2>&1',
    ],
  );
});

test('does not save a session when Git setup fails', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const harness = createDoricTestHarness({
    sessions,
    hasGit: false,
    gitInstallFailure: new Error('apt failed'),
  });

  await assert.rejects(
    harness.executor.execute(
      createRequestContext({
        contextId: 'context-1',
      }),
      createEventBus(),
    ),
    /git install failed: apt failed/u,
  );

  assert.equal(sessions.get('context-1'), undefined);
  assert.deepEqual(sessions.list(), []);
  assert.equal(harness.sandboxes[0]?.clones.length, 0);
});

test('does not save a session when cloning fails', async () => {
  const sessions = createSessionStore<DoricSessionContext>();
  const cloneFailure = new Error('clone failed');
  const harness = createDoricTestHarness({ sessions, cloneFailure });

  await assert.rejects(
    harness.executor.execute(
      createRequestContext({
        contextId: 'context-1',
      }),
      createEventBus(),
    ),
    /clone failed/u,
  );

  assert.equal(sessions.get('context-1'), undefined);
  assert.deepEqual(sessions.list(), []);
});

const jwt = (claims: Record<string, unknown>): string =>
  [
    'header',
    Buffer.from(JSON.stringify(claims)).toString('base64url'),
    'signature',
  ].join('.');
