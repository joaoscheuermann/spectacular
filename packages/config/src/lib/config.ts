import { ConfigParseError } from './classes/parse-error.js';
import type {
  AgentConfig,
  GithubConfig,
  ModelConfig,
  ProviderConfig,
  TaskConfig,
} from './types/config.js';
import type { ConfigParseErrorCode } from './types/error.js';
import { at, isRecord } from './utils/object.js';

export { ConfigParseError } from './classes/parse-error.js';
export type {
  AgentConfig,
  ConfigDataPart,
  GithubConfig,
  InitialMessage,
  ModelConfig,
  ProviderConfig,
  TaskConfig,
} from './types/config.js';
export type { ConfigParseErrorCode, ConfigParseIssue } from './types/error.js';

/** Parses the config data from the first part of an initial agent message. */
export function parseInitialMessageConfig(message: unknown): AgentConfig {
  if (!isRecord(message)) {
    throw invalid(
      'invalid_message',
      'message',
      'Initial message must be an object',
    );
  }

  const parts = message['parts'];

  if (!Array.isArray(parts) || parts.length === 0) {
    throw invalid(
      'missing_first_part',
      'message.parts[0]',
      'Initial message must start with a config data part',
    );
  }

  const firstPart = parts[0];

  if (!isRecord(firstPart) || firstPart['kind'] !== 'data') {
    throw invalid(
      'invalid_first_part_kind',
      'message.parts[0].kind',
      'First message part must be a data part',
    );
  }

  const dataPath = 'message.parts[0].data';
  const data = object(firstPart['data'], dataPath);
  const type = string(
    data['type'],
    at(dataPath, 'type'),
    'invalid_config_type',
  );

  if (type !== 'config') {
    throw invalid(
      'invalid_config_type',
      at(dataPath, 'type'),
      'Config data type must be "config"',
    );
  }

  return parseConfig(data['data'], at(dataPath, 'data'));
}

/** Parses an agent config payload and validates every supported config field. */
export function parseConfig(value: unknown, path = 'config'): AgentConfig {
  const input = object(value, path);

  return {
    github: parseGithub(input['github'], at(path, 'github')),
    providers: array(input['providers'], at(path, 'providers')).map(
      (provider, index) =>
        parseProvider(provider, at(at(path, 'providers'), index)),
    ),
    models: array(input['models'], at(path, 'models')).map((model, index) =>
      parseModel(model, at(at(path, 'models'), index)),
    ),
    tasks: array(input['tasks'], at(path, 'tasks')).map((task, index) =>
      parseTask(task, at(at(path, 'tasks'), index)),
    ),
  };
}

function parseGithub(value: unknown, path: string): GithubConfig {
  const input = object(value, path);
  const repo = object(input['repo'], at(path, 'repo'));

  return {
    repo: {
      url: string(repo['url'], at(at(path, 'repo'), 'url')),
    },
    token: string(input['token'], at(path, 'token')),
  };
}

function parseProvider(value: unknown, path: string): ProviderConfig {
  const input = object(value, path);

  return {
    id: string(input['id'], at(path, 'id')),
    type: string(input['type'], at(path, 'type')),
    token: string(input['token'], at(path, 'token')),
  };
}

function parseModel(value: unknown, path: string): ModelConfig {
  const input = object(value, path);
  const reasoning = optionalString(input['reasoning'], at(path, 'reasoning'));
  const internalKey = optionalString(
    input['internal_key'],
    at(path, 'internal_key'),
  );

  return {
    id: string(input['id'], at(path, 'id')),
    provider: string(input['provider'], at(path, 'provider')),
    model: string(input['model'], at(path, 'model')),
    ...(reasoning === undefined ? {} : { reasoning }),
    ...(internalKey === undefined ? {} : { internal_key: internalKey }),
  };
}

function parseTask(value: unknown, path: string): TaskConfig {
  const input = object(value, path);

  return {
    id: string(input['id'], at(path, 'id')),
    model: string(input['model'], at(path, 'model')),
  };
}

function object(value: unknown, path: string): Record<string, unknown> {
  if (!isRecord(value)) {
    throw invalid('invalid_config_field', path, 'Expected an object');
  }

  return value;
}

function array(value: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(value)) {
    throw invalid('invalid_config_field', path, 'Expected an array');
  }

  return value;
}

function string(
  value: unknown,
  path: string,
  code: ConfigParseErrorCode = 'invalid_config_field',
): string {
  if (typeof value !== 'string') {
    throw invalid(code, path, 'Expected a string');
  }

  return value;
}

function optionalString(value: unknown, path: string): string | undefined {
  if (value === undefined) {
    return undefined;
  }

  return string(value, path);
}

function invalid(
  code: ConfigParseErrorCode,
  path: string,
  message: string,
): ConfigParseError {
  return new ConfigParseError(code, path, message);
}
