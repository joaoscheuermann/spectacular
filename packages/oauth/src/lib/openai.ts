import { createOAuthClient } from './oauth.js';
import type { CreateOAuthClientOptions, OAuthClient, OAuthProfile } from './types/oauth.js';

export const openAiOAuthProfile: OAuthProfile = {
  provider: 'openai',
  profile: 'openai',
  authorizationEndpoint: 'https://auth.openai.com/oauth/authorize',
  tokenEndpoint: 'https://auth.openai.com/oauth/token',
  defaultScope: 'openid profile email offline_access',
  defaultTokenScheme: 'Bearer',
};

export type CreateOpenAiOAuthOptions = Omit<CreateOAuthClientOptions, 'profile'> & {
  readonly profile?: Partial<OAuthProfile>;
};

/** Creates an OpenAI OAuth client with OpenAI endpoint and scope defaults. */
export const createOpenAiOAuth = (
  options: CreateOpenAiOAuthOptions,
): OAuthClient =>
  createOAuthClient({
    ...options,
    profile: {
      ...openAiOAuthProfile,
      ...options.profile,
    },
  });
