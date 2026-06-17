import { openAiMetadata } from './providers/openai.js';
import { openRouterMetadata } from './providers/openrouter.js';
import type { ProviderMetadata } from './types/provider.js';

export const providerRegistry = [openAiMetadata, openRouterMetadata] as const;

export const providerById = (id: string): ProviderMetadata | undefined =>
  providerRegistry.find((provider) => provider.id === id);

export type ProviderAvailability = {
  readonly openRouterApiKey?: string;
  readonly openAiApiKey?: string;
  readonly openAiOAuth?: boolean;
};

export const enabledProviderName = (
  availability: ProviderAvailability,
): string | undefined => {
  if (availability.openRouterApiKey !== undefined && availability.openRouterApiKey !== '') {
    return 'openrouter';
  }

  if (
    (availability.openAiApiKey !== undefined && availability.openAiApiKey !== '') ||
    availability.openAiOAuth === true
  ) {
    return 'openai';
  }

  return undefined;
};
