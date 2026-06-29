import { ProviderErrorObject } from '../../classes/provider-error.js';

export type SecretSource = string | (() => string | Promise<string>);

export type AuthDeps = {
  readonly apiKey?: SecretSource;
  readonly authorization?: SecretSource;
};

export const authorization = async (
  deps: AuthDeps,
): Promise<string | undefined> => {
  const apiKey = await secret(deps.apiKey);
  const auth = await secret(deps.authorization);

  if (apiKey !== undefined && auth !== undefined) {
    throw new ProviderErrorObject({
      provider: 'openai',
      code: 'auth_ambiguous',
      message:
        'OpenAI provider accepts either apiKey or authorization, not both.',
    });
  }

  if (apiKey !== undefined) {
    return `Bearer ${apiKey}`;
  }

  return auth;
};

const secret = async (
  source: SecretSource | undefined,
): Promise<string | undefined> => {
  if (source === undefined) {
    return undefined;
  }

  const value = typeof source === 'function' ? await source() : source;

  if (value.trim() === '') {
    return undefined;
  }

  return value;
};
