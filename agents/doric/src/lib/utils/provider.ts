import { A2AError } from '@a2a-js/sdk/server';

import type { ProviderConfig } from 'config';
import {
  createFetchTransport as createLlmFetchTransport,
  createCodexProvider,
  createLmStudioProvider,
  createOpenAiProvider,
  createOpenRouterProvider,
} from 'llms';
import type { LlmProvider } from 'llms';

import { codexCredentialFromToken } from 'oauth';

export const createProviderFromConfig = async (
  provider: ProviderConfig,
): Promise<LlmProvider> => {
  const transport = createLlmFetchTransport();

  if (provider.type === 'openai') {
    const token = optionalToken(provider);

    if (token === undefined) {
      return createOpenAiProvider({ transport });
    }

    return token.toLowerCase().startsWith('bearer ')
      ? createOpenAiProvider({ transport, authorization: token })
      : createOpenAiProvider({ transport, apiKey: token });
  }

  if (provider.type === 'lmstudio') {
    const token = optionalToken(provider);
    const baseUrl =
      provider.baseUrl === undefined ? {} : { baseUrl: provider.baseUrl };

    if (token === undefined) {
      return createLmStudioProvider({ transport, ...baseUrl });
    }

    return token.toLowerCase().startsWith('bearer ')
      ? createLmStudioProvider({ transport, ...baseUrl, authorization: token })
      : createLmStudioProvider({ transport, ...baseUrl, apiKey: token });
  }

  if (provider.type === 'openrouter') {
    return createOpenRouterProvider({
      transport,
      apiKey: bearerValue(requiredToken(provider)),
    });
  }

  if (provider.type === 'codex') {
    const credential = codexCredentialFromToken(requiredToken(provider));

    return createCodexProvider({
      transport,
      authorization: credential.authorization,
      chatGptAccountId: credential.accountId,
      fedramp: credential.fedramp,
    });
  }

  throw A2AError.invalidParams(
    `Unsupported provider type "${provider.type}".`,
    {
      code: 'unsupported_provider_type',
      path: 'message.metadata.configuration.providers.type',
    },
  );
};

const optionalToken = (provider: ProviderConfig): string | undefined => {
  const token = provider.token?.trim();

  return token === undefined || token === '' ? undefined : token;
};

const requiredToken = (provider: ProviderConfig): string => {
  const token = optionalToken(provider);

  if (token === undefined) {
    throw A2AError.invalidParams(
      `Provider "${provider.id}" requires a token.`,
      {
        code: 'missing_provider_token',
        path: 'message.metadata.configuration.providers.token',
      },
    );
  }

  return token;
};

const bearerValue = (token: string): string => {
  const trimmed = token.trim();

  return trimmed.toLowerCase().startsWith('bearer ')
    ? trimmed.slice('bearer '.length).trimStart()
    : trimmed;
};
