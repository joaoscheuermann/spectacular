import { codexMetadata } from './providers/codex.js';
import { openAiMetadata } from './providers/openai.js';
import { openRouterMetadata } from './providers/openrouter.js';
import type { ProviderMetadata } from './types/provider.js';

export const providerRegistry = [
  codexMetadata,
  openAiMetadata,
  openRouterMetadata,
] as const;

export const providerById = (id: string): ProviderMetadata | undefined =>
  providerRegistry.find((provider) => provider.id === id);

export type ProviderAvailability = {
  readonly openRouterApiKey?: string;
  readonly openAiApiKey?: string;
  readonly codexAuth?: string;
};

export const enabledProviderName = (
  availability: ProviderAvailability,
): string | undefined => {
  if (availability.codexAuth !== undefined && availability.codexAuth !== '') {
    return 'codex';
  }

  if (
    availability.openRouterApiKey !== undefined &&
    availability.openRouterApiKey !== ''
  ) {
    return 'openrouter';
  }

  if (
    availability.openAiApiKey !== undefined &&
    availability.openAiApiKey !== ''
  ) {
    return 'openai';
  }

  return undefined;
};
