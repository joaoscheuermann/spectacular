import assert from 'node:assert/strict';
import test from 'node:test';

import {
  ConfigParseError,
  parseConfig,
  parseInitialMessageConfig,
  parseMessageConfigUpdate,
  type AgentConfig,
} from '../src/index.js';

test('returns config data when message metadata contains configuration', () => {
  const config = sampleConfig();

  assert.deepEqual(
    parseInitialMessageConfig({
      metadata: { configuration: config },
      parts: [{ kind: 'text', text: 'Build the agent' }],
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

test('parses optional GitHub repository branch when provided', () => {
  const config = sampleConfig({
    github: {
      repo: {
        url: 'https://github.com/example/repo',
        branch: 'feature/doric-cli',
      },
      token: 'github-token',
    },
  });

  assert.deepEqual(parseConfig(config).github.repo, {
    url: 'https://github.com/example/repo',
    branch: 'feature/doric-cli',
  });
});

test('returns config update when later message metadata contains configuration', () => {
  const config = sampleConfig();

  assert.deepEqual(
    parseMessageConfigUpdate({
      metadata: { configuration: config },
      parts: [{ kind: 'text', text: 'Continue' }],
    }),
    config,
  );
});

test('returns undefined when later message omits metadata configuration', () => {
  assert.equal(
    parseMessageConfigUpdate({
      metadata: {},
      parts: [{ kind: 'text', text: 'Continue' }],
    }),
    undefined,
  );
});

test('rejects messages without metadata configuration', () => {
  assertConfigError(
    () => parseInitialMessageConfig({ parts: [] }),
    'missing_configuration',
    'message.metadata.configuration',
  );
});

test('rejects metadata without configuration', () => {
  assertConfigError(
    () =>
      parseInitialMessageConfig({
        metadata: {},
        parts: [{ kind: 'text', text: 'hello' }],
      }),
    'missing_configuration',
    'message.metadata.configuration',
  );
});

test('rejects non-object metadata configuration', () => {
  assertConfigError(
    () =>
      parseInitialMessageConfig({
        metadata: { configuration: 'not an object' },
        parts: [{ kind: 'text', text: 'hello' }],
      }),
    'invalid_config_field',
    'message.metadata.configuration',
  );
});

test('rejects first-part config data when metadata configuration is missing', () => {
  assertConfigError(
    () =>
      parseInitialMessageConfig({
        parts: [
          { kind: 'data', data: { type: 'config', data: sampleConfig() } },
        ],
      }),
    'missing_configuration',
    'message.metadata.configuration',
  );
});

test('rejects malformed config fields with the invalid field path', () => {
  const config = sampleConfig();

  assertConfigError(
    () =>
      parseInitialMessageConfig({
        metadata: {
          configuration: {
            ...config,
            models: [{ id: 'planning', provider: 'openai', model: 1 }],
          },
        },
        parts: [{ kind: 'text', text: 'Build the agent' }],
      }),
    'invalid_config_field',
    'message.metadata.configuration.models[0].model',
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
