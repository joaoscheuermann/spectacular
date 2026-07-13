import {
  createCodexProvider,
  createFetchTransport,
  createLmStudioOpenAiProvider,
  createLmStudioProvider,
  createOpenAiProvider,
  createOpenRouterProvider,
  type HttpTransport,
  type LlmProvider,
  type ProviderRequest,
  type StructuredOutputSchema,
} from 'llms';
import { codexCredentialFromToken } from 'oauth';
import type { z } from 'zod';

import type {
  EvolutionConfig,
  ModelRef,
  ProviderConfig,
  ProviderType,
} from './schema.js';

export type Environment = Readonly<Record<string, string | undefined>>;

export type Completion = {
  readonly text: (system: string, input: string) => Promise<string>;
  readonly structured: <Schema extends StructuredOutputSchema>(
    system: string,
    input: string,
    schema: Schema,
  ) => Promise<z.output<Schema>>;
};

export type CompletionFor = (model: ModelRef) => Completion;

const defaultTokenEnvs: Readonly<Record<ProviderType, string>> = {
  openai: 'OPENAI_API_KEY',
  openrouter: 'OPENROUTER_API_KEY',
  lmstudio: 'LM_STUDIO_API_KEY',
  'lmstudio-openai': 'LM_STUDIO_API_KEY',
  codex: 'CODEX_AUTHORIZATION',
};

const token = (
  config: ProviderConfig,
  env: Environment,
): string | undefined => {
  const value = env[config.tokenEnv ?? defaultTokenEnvs[config.type]]?.trim();
  return value === '' ? undefined : value;
};

const requiredToken = (config: ProviderConfig, env: Environment): string => {
  const value = token(config, env);
  if (value !== undefined) return value;
  throw new Error(
    'Provider "' +
      config.id +
      '" requires environment variable ' +
      (config.tokenEnv ?? defaultTokenEnvs[config.type]) +
      '.',
  );
};

const bearerValue = (value: string): string =>
  value.toLowerCase().startsWith('bearer ')
    ? value.slice('bearer '.length).trimStart()
    : value;

const optionalAuth = (
  value: string | undefined,
): { readonly apiKey?: string; readonly authorization?: string } => {
  if (value === undefined) return {};
  return value.toLowerCase().startsWith('bearer ')
    ? { authorization: value }
    : { apiKey: value };
};

/** Composes one public llms provider without persisting resolved credentials. */
export const createProvider = (
  config: ProviderConfig,
  env: Environment = process.env,
  transport: HttpTransport = createFetchTransport(),
): LlmProvider => {
  const baseUrl =
    config.baseUrl === undefined ? {} : { baseUrl: config.baseUrl };

  if (config.type === 'openai') {
    return createOpenAiProvider({
      transport,
      ...baseUrl,
      ...optionalAuth(token(config, env)),
    });
  }

  if (config.type === 'openrouter') {
    return createOpenRouterProvider({
      transport,
      ...baseUrl,
      apiKey: bearerValue(requiredToken(config, env)),
    });
  }

  if (config.type === 'lmstudio') {
    return createLmStudioProvider({
      transport,
      ...baseUrl,
      ...optionalAuth(token(config, env)),
    });
  }

  if (config.type === 'lmstudio-openai') {
    return createLmStudioOpenAiProvider({
      transport,
      ...baseUrl,
      ...optionalAuth(token(config, env)),
    });
  }

  const credential = codexCredentialFromToken(requiredToken(config, env));
  return createCodexProvider({
    transport,
    ...baseUrl,
    authorization: credential.authorization,
    ...(credential.accountId === undefined
      ? {}
      : { chatGptAccountId: credential.accountId }),
    ...(credential.fedramp === undefined
      ? {}
      : { fedramp: credential.fedramp }),
  });
};

const request = (
  model: ModelRef,
  system: string,
  input: string,
): ProviderRequest => ({
  model: model.model,
  messages: [
    { role: 'system', content: system },
    { role: 'user', content: input },
  ],
  ...(model.effort === undefined ? {} : { effort: model.effort }),
  ...(model.temperature === undefined
    ? {}
    : { temperature: model.temperature }),
  ...(model.maxOutputTokens === undefined
    ? {}
    : { maxOutputTokens: model.maxOutputTokens }),
});

const completion = (provider: LlmProvider, model: ModelRef): Completion => ({
  async text(system, input) {
    return (await provider.complete(request(model, system, input))).text;
  },

  async structured(system, input, schema) {
    const response = await provider.complete({
      ...request(model, system, input),
      schema,
    });
    if (response.structured === undefined) {
      throw new Error(
        'Provider "' +
          provider.metadata.id +
          '" returned no structured output for model "' +
          model.model +
          '".',
      );
    }
    return schema.parse(response.structured);
  },
});

/** Resolves model references to cached provider instances for one run. */
export const createCompletionFor = (
  config: EvolutionConfig,
  env: Environment = process.env,
  transport: HttpTransport = createFetchTransport(),
): CompletionFor => {
  const configs = new Map(
    config.providers.map((provider) => [provider.id, provider]),
  );
  const providers = new Map<string, LlmProvider>();

  return (model) => {
    const providerConfig = configs.get(model.provider);
    if (providerConfig === undefined) {
      throw new Error('Unknown provider reference: ' + model.provider);
    }
    const provider =
      providers.get(model.provider) ??
      createProvider(providerConfig, env, transport);
    providers.set(model.provider, provider);
    return completion(provider, model);
  };
};
