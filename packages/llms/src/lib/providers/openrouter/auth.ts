import { ProviderErrorObject } from '../../classes/provider-error.js';

type SecretSource = string | (() => string | Promise<string>);

export const authorization = async (apiKey: SecretSource): Promise<string> =>
  `Bearer ${await secret(apiKey)}`;

const secret = async (source: SecretSource): Promise<string> => {
  const value = typeof source === 'function' ? await source() : source;

  if (value.trim() === '') {
    throw new ProviderErrorObject({
      provider: 'openrouter',
      code: 'auth_missing',
      message: 'OpenRouter provider requires an API key.',
    });
  }

  return value;
};
