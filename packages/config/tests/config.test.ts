import assert from 'node:assert/strict';
import test from 'node:test';

import {
  ConfigParseError,
  parseConfig,
  parseInitialMessageConfig,
  type AgentConfig,
} from '../src/index.js';

test('returns config data when the first message part is config data', () => {
  const config = sampleConfig();

  assert.deepEqual(
    parseInitialMessageConfig({
      parts: [
        { kind: 'data', data: { type: 'config', data: config } },
        { kind: 'text', text: 'Build the agent' },
      ],
    }),
    config,
  );
});

test('parses config objects directly when the payload is valid', () => {
  const config = sampleConfig({
    models: [
      {
        id: 'planning',
        provider: 'openai',
        model: 'gpt-5',
        reasoning: 'medium',
        internal_key: 'planner',
      },
    ],
  });

  assert.deepEqual(parseConfig(config), config);
});

test('rejects messages without a first part', () => {
  assertConfigError(
    () => parseInitialMessageConfig({ parts: [] }),
    'missing_first_part',
    'message.parts[0]',
  );
});

test('rejects messages whose first part is not data', () => {
  assertConfigError(
    () =>
      parseInitialMessageConfig({ parts: [{ kind: 'text', text: 'hello' }] }),
    'invalid_first_part_kind',
    'message.parts[0].kind',
  );
});

test('rejects first data parts whose internal type is not config', () => {
  assertConfigError(
    () =>
      parseInitialMessageConfig({
        parts: [
          { kind: 'data', data: { type: 'prompt', data: sampleConfig() } },
        ],
      }),
    'invalid_config_type',
    'message.parts[0].data.type',
  );
});

test('rejects config data parts without a nested config payload', () => {
  assertConfigError(
    () =>
      parseInitialMessageConfig({
        parts: [{ kind: 'data', data: { type: 'config' } }],
      }),
    'invalid_config_field',
    'message.parts[0].data.data',
  );
});

test('rejects malformed config fields with the invalid field path', () => {
  const config = sampleConfig();

  assertConfigError(
    () =>
      parseInitialMessageConfig({
        parts: [
          {
            kind: 'data',
            data: {
              type: 'config',
              data: {
                ...config,
                models: [{ id: 'planning', provider: 'openai', model: 1 }],
              },
            },
          },
        ],
      }),
    'invalid_config_field',
    'message.parts[0].data.data.models[0].model',
  );
});

function assertConfigError(
  action: () => unknown,
  code: ConfigParseError['code'],
  path: string,
): void {
  assert.throws(action, (error: unknown) => {
    assert.ok(error instanceof ConfigParseError);
    assert.equal(error.code, code);
    assert.equal(error.path, path);
    return true;
  });
}

function sampleConfig(overrides: Partial<AgentConfig> = {}): AgentConfig {
  return {
    github: {
      repo: {
        url: 'https://github.com/example/repo',
      },
      token: 'github-token',
    },
    providers: [
      {
        id: 'openai',
        type: 'openai',
        token: 'provider-token',
      },
    ],
    models: [
      {
        id: 'default',
        provider: 'openai',
        model: 'gpt-5',
      },
    ],
    tasks: [
      {
        id: 'develop',
        model: 'default',
      },
    ],
    ...overrides,
  };
}
