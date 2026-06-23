import { A2AError } from '@a2a-js/sdk/server';

import type { ProviderConfig } from 'config';
import {
  createFetchTransport as createLlmFetchTransport,
  createCodexProvider,
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
    const token = requiredToken(provider);

    return token.toLowerCase().startsWith('bearer ')
      ? createOpenAiProvider({ transport, authorization: token })
      : createOpenAiProvider({ transport, apiKey: token });
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

const requiredToken = (provider: ProviderConfig): string => {
  const token = provider.token?.trim();

  if (token === undefined || token === '') {
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
