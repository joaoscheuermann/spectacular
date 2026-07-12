import assert from 'node:assert/strict';
import test from 'node:test';

import {
  createCompletionFor,
  createProvider,
  type Environment,
} from '../src/completion.js';
import type { HttpRequest, HttpTransport } from 'llms';
import { resultJudgeSystemPrompt } from '../src/prompts.js';
import {
  judgmentSchema,
  type EvolutionConfig,
  type ProviderConfig,
} from '../src/schema.js';

const transport = (requests: HttpRequest[]): HttpTransport => ({
  async request(request) {
    requests.push(request);
    return {
      status: 200,
      headers: {},
      body: request.url.includes('/api/v1/models')
        ? '{"models":[]}'
        : '{"data":[]}',
    };
  },
  async *stream() {
    return;
  },
});

test('composes every public provider branch and forwards base URLs', async () => {
  const requests: HttpRequest[] = [];
  const env: Environment = {
    TOKEN: 'Bearer secret',
    CODEX: 'Bearer codex-secret',
  };
  const configs: readonly ProviderConfig[] = [
    {
      id: 'openai',
      type: 'openai',
      baseUrl: 'https://example.test/openai',
      tokenEnv: 'TOKEN',
    },
    {
      id: 'openrouter',
      type: 'openrouter',
      baseUrl: 'https://example.test/openrouter',
      tokenEnv: 'TOKEN',
    },
    {
      id: 'lmstudio',
      type: 'lmstudio',
      baseUrl: 'https://example.test/lmstudio',
      tokenEnv: 'TOKEN',
    },
    {
      id: 'lmstudio-openai',
      type: 'lmstudio-openai',
      baseUrl: 'https://example.test/lmstudio-openai',
      tokenEnv: 'TOKEN',
    },
    {
      id: 'codex',
      type: 'codex',
      baseUrl: 'https://example.test/codex',
      tokenEnv: 'CODEX',
    },
  ];

  for (const config of configs) {
    const provider = createProvider(config, env, transport(requests));
    assert.equal(provider.metadata.id, config.type);
    await provider.models();
  }

  assert.deepEqual(
    requests.map(({ url }) => url),
    [
      'https://example.test/openai/models',
      'https://example.test/openrouter/models',
      'https://example.test/lmstudio/api/v1/models',
      'https://example.test/lmstudio-openai/models',
      'https://example.test/codex/models',
    ],
  );
  assert.ok(
    requests.every(
      ({ headers }) => headers?.authorization?.startsWith('Bearer ') === true,
    ),
  );
});

test('requires runtime secrets only for OpenRouter and Codex', () => {
  const fake = transport([]);
  assert.doesNotThrow(() =>
    createProvider({ id: 'openai', type: 'openai' }, {}, fake),
  );
  assert.doesNotThrow(() =>
    createProvider({ id: 'local', type: 'lmstudio' }, {}, fake),
  );
  assert.doesNotThrow(() =>
    createProvider({ id: 'local', type: 'lmstudio-openai' }, {}, fake),
  );
  assert.throws(
    () => createProvider({ id: 'router', type: 'openrouter' }, {}, fake),
    /OPENROUTER_API_KEY/,
  );
  assert.throws(
    () => createProvider({ id: 'codex', type: 'codex' }, {}, fake),
    /CODEX_AUTHORIZATION/,
  );
});

test('parses native LM Studio JSON text for structured evolution roles', async () => {
  const requests: HttpRequest[] = [];
  const nativeTransport: HttpTransport = {
    async request(request) {
      requests.push(request);
      return {
        status: 200,
        headers: {},
        body: JSON.stringify({
          output: [
            {
              type: 'message',
              content: JSON.stringify({
                passed: true,
                ambiguous: false,
                rationale: 'Clear.',
              }),
            },
          ],
        }),
      };
    },
    async *stream() {
      return;
    },
  };
  const config: EvolutionConfig = {
    providers: [{ id: 'native', type: 'lmstudio' }],
    models: [{ id: 'native-model', provider: 'native', model: 'model' }],
    optimizer: { provider: 'native', model: 'model' },
    judges: [
      { provider: 'native', model: 'judge-one' },
      { provider: 'native', model: 'judge-two' },
    ],
    evolution: {
      targetAccuracy: 1,
      plateauPatience: 1,
      maxEpochs: 1,
    },
  };

  const judgment = await createCompletionFor(
    config,
    {},
    nativeTransport,
  )(config.judges[0]).structured(
    resultJudgeSystemPrompt,
    'Judge this.',
    judgmentSchema,
  );

  assert.equal(judgment.passed, true);
  const body = JSON.parse(requests[0]?.body ?? '{}') as {
    readonly system_prompt?: string;
  };
  assert.match(
    body.system_prompt ?? '',
    /Return only JSON with passed:boolean, ambiguous:boolean, and rationale:string/,
  );
});
