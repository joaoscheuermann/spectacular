import { ProviderErrorObject } from '../../classes/provider-error.js';

export type SecretSource = string | (() => string | Promise<string>);

export type AuthDeps = {
  readonly apiKey?: SecretSource;
  readonly authorization?: SecretSource;
};

export const authorization = async (deps: AuthDeps): Promise<string> => {
  if (deps.apiKey !== undefined && deps.authorization !== undefined) {
    throw new ProviderErrorObject({
      provider: 'openai',
      code: 'auth_ambiguous',
      message:
        'OpenAI provider accepts either apiKey or authorization, not both.',
    });
  }

  if (deps.apiKey !== undefined) {
    return `Bearer ${await secret(deps.apiKey)}`;
  }

  if (deps.authorization !== undefined) {
    return secret(deps.authorization);
  }

  throw new ProviderErrorObject({
    provider: 'openai',
    code: 'auth_missing',
    message: 'OpenAI provider requires an API key or authorization header.',
  });
};

const secret = async (source: SecretSource): Promise<string> => {
  const value = typeof source === 'function' ? await source() : source;

  if (value.trim() === '') {
    throw new ProviderErrorObject({
      provider: 'openai',
      code: 'auth_missing',
      message: 'OpenAI provider received an empty auth value.',
    });
  }

  return value;
};
