import { createHash, randomBytes } from 'node:crypto';

import { ProviderErrorObject } from '../classes/provider-error.js';
import type { HttpTransport } from '../types/http.js';
import { asRecord, numberField, stringField } from '../utils/json.js';

export type OpenAiAuthRecord = {
  readonly accessToken: string;
  readonly refreshToken?: string;
  readonly expiresAt?: number;
  readonly tokenType?: string;
  readonly scope?: string;
  readonly claims?: Readonly<Record<string, unknown>>;
};

export interface OpenAiAuthStore {
  load(): Promise<OpenAiAuthRecord | undefined>;
  save(record: OpenAiAuthRecord): Promise<void>;
  clear?(): Promise<void>;
}

export type BrowserOpener = (url: string) => Promise<void> | void;

export type RandomSource = {
  bytes(length: number): Uint8Array;
};

export type OpenAiOAuthCallback = {
  readonly code?: string;
  readonly state?: string;
  readonly error?: string;
  readonly errorDescription?: string;
};

export type OpenAiOAuthCallbackServer = {
  waitForCallback(
    expectedState: string,
    signal?: AbortSignal,
  ): Promise<OpenAiOAuthCallback>;
};

export type OpenAiOAuthFlowDeps = {
  readonly transport: HttpTransport;
  readonly authStore: OpenAiAuthStore;
  readonly clientId: string;
  readonly redirectUri: string;
  readonly scope?: string;
  readonly authorizationEndpoint?: string;
  readonly tokenEndpoint?: string;
  readonly browserOpener: BrowserOpener;
  readonly callbackServer: OpenAiOAuthCallbackServer;
  readonly random?: RandomSource;
  readonly clock?: () => number;
  readonly signal?: AbortSignal;
};

export type OpenAiAuthHeaderDeps = {
  readonly transport: HttpTransport;
  readonly authStore: OpenAiAuthStore;
  readonly clientId?: string;
  readonly clientSecret?: string;
  readonly tokenEndpoint?: string;
  readonly refreshSkewMs?: number;
  readonly clock?: () => number;
};

const DEFAULT_AUTH_URL = 'https://auth.openai.com/oauth/authorize';
const DEFAULT_TOKEN_URL = 'https://auth.openai.com/oauth/token';
const DEFAULT_SCOPE = 'openid profile email offline_access';
const DEFAULT_REFRESH_SKEW_MS = 60_000;

export const nodeRandomSource: RandomSource = {
  bytes(length: number): Uint8Array {
    return randomBytes(length);
  },
};

export const parseOpenAiTokenClaims = (
  token: string,
): Readonly<Record<string, unknown>> | undefined => {
  const [, payload] = token.split('.');

  if (payload === undefined) {
    return undefined;
  }

  try {
    return asRecord(JSON.parse(Buffer.from(payload, 'base64url').toString('utf8')));
  } catch {
    return undefined;
  }
};

export async function runOpenAiOAuthCallbackFlow(
  deps: OpenAiOAuthFlowDeps,
): Promise<OpenAiAuthRecord> {
  const random = deps.random ?? nodeRandomSource;
  const now = deps.clock ?? Date.now;
  const verifier = token(random, 32);
  const state = token(random, 16);
  const challenge = base64Url(
    createHash('sha256').update(verifier).digest(),
  );
  const authUrl = new URL(deps.authorizationEndpoint ?? DEFAULT_AUTH_URL);
  authUrl.searchParams.set('response_type', 'code');
  authUrl.searchParams.set('client_id', deps.clientId);
  authUrl.searchParams.set('redirect_uri', deps.redirectUri);
  authUrl.searchParams.set('scope', deps.scope ?? DEFAULT_SCOPE);
  authUrl.searchParams.set('state', state);
  authUrl.searchParams.set('code_challenge', challenge);
  authUrl.searchParams.set('code_challenge_method', 'S256');

  await deps.browserOpener(authUrl.toString());

  const callback = await deps.callbackServer.waitForCallback(state, deps.signal);

  if (callback.error !== undefined) {
    throw authError(
      'oauth_callback_error',
      callback.errorDescription ?? callback.error,
    );
  }

  if (callback.state !== state) {
    throw authError('oauth_state_mismatch', 'OAuth callback state did not match.');
  }

  if (callback.code === undefined || callback.code === '') {
    throw authError('oauth_missing_code', 'OAuth callback did not include a code.');
  }

  const record = await exchangeToken(deps.transport, {
    clientId: deps.clientId,
    code: callback.code,
    codeVerifier: verifier,
    redirectUri: deps.redirectUri,
    tokenEndpoint: deps.tokenEndpoint ?? DEFAULT_TOKEN_URL,
    now,
  });

  await deps.authStore.save(record);

  return record;
}

export async function getOpenAiAuthHeader(
  deps: OpenAiAuthHeaderDeps,
  options: { readonly forceRefresh?: boolean } = {},
): Promise<string> {
  const record = await deps.authStore.load();

  if (record === undefined) {
    throw authError('oauth_missing_record', 'OpenAI OAuth credentials are missing.');
  }

  if (!options.forceRefresh && !shouldRefresh(record, deps)) {
    return bearer(record);
  }

  return bearer(await refreshOpenAiAuthRecord(deps, record));
}

export async function refreshOpenAiAuthRecord(
  deps: OpenAiAuthHeaderDeps,
  current?: OpenAiAuthRecord,
): Promise<OpenAiAuthRecord> {
  const existing = current ?? (await deps.authStore.load());

  if (existing?.refreshToken === undefined) {
    throw authError('oauth_missing_refresh_token', 'OpenAI refresh token is missing.');
  }

  const body = new URLSearchParams({
    grant_type: 'refresh_token',
    refresh_token: existing.refreshToken,
  });

  if (deps.clientId !== undefined) {
    body.set('client_id', deps.clientId);
  }

  if (deps.clientSecret !== undefined) {
    body.set('client_secret', deps.clientSecret);
  }

  const response = await deps.transport.request({
    method: 'POST',
    url: deps.tokenEndpoint ?? DEFAULT_TOKEN_URL,
    headers: {
      'content-type': 'application/x-www-form-urlencoded',
      accept: 'application/json',
    },
    body: body.toString(),
  });

  if (response.status >= 400) {
    throw authError('oauth_refresh_failed', response.body, response.status);
  }

  const next = tokenResponse(response.body, deps.clock ?? Date.now);
  const saved = {
    ...next,
    refreshToken: next.refreshToken ?? existing.refreshToken,
  };

  await deps.authStore.save(saved);

  return saved;
}

const exchangeToken = async (
  transport: HttpTransport,
  request: {
    readonly clientId: string;
    readonly code: string;
    readonly codeVerifier: string;
    readonly redirectUri: string;
    readonly tokenEndpoint: string;
    readonly now: () => number;
  },
): Promise<OpenAiAuthRecord> => {
  const body = new URLSearchParams({
    grant_type: 'authorization_code',
    client_id: request.clientId,
    code: request.code,
    code_verifier: request.codeVerifier,
    redirect_uri: request.redirectUri,
  });

  const response = await transport.request({
    method: 'POST',
    url: request.tokenEndpoint,
    headers: {
      'content-type': 'application/x-www-form-urlencoded',
      accept: 'application/json',
    },
    body: body.toString(),
  });

  if (response.status >= 400) {
    throw authError('oauth_token_exchange_failed', response.body, response.status);
  }

  return tokenResponse(response.body, request.now);
};

const tokenResponse = (body: string, now: () => number): OpenAiAuthRecord => {
  const parsed = asRecord(JSON.parse(body));

  if (parsed === undefined) {
    throw authError('oauth_invalid_token_response', 'Token response was not an object.');
  }

  const accessToken = stringField(parsed, 'access_token');

  if (accessToken === undefined) {
    throw authError('oauth_invalid_token_response', 'Token response lacked access_token.');
  }

  const expiresIn = numberField(parsed, 'expires_in');

  return {
    accessToken,
    refreshToken: stringField(parsed, 'refresh_token'),
    expiresAt: expiresIn === undefined ? undefined : now() + expiresIn * 1000,
    tokenType: stringField(parsed, 'token_type'),
    scope: stringField(parsed, 'scope'),
    claims: parseOpenAiTokenClaims(accessToken),
  };
};

const shouldRefresh = (
  record: OpenAiAuthRecord,
  deps: OpenAiAuthHeaderDeps,
): boolean => {
  if (record.expiresAt === undefined) {
    return false;
  }

  const now = deps.clock ?? Date.now;
  const skew = deps.refreshSkewMs ?? DEFAULT_REFRESH_SKEW_MS;

  return record.expiresAt - now() <= skew;
};

const bearer = (record: OpenAiAuthRecord): string =>
  `${record.tokenType ?? 'Bearer'} ${record.accessToken}`;

const token = (random: RandomSource, length: number): string =>
  base64Url(random.bytes(length));

const base64Url = (value: Uint8Array): string =>
  Buffer.from(value).toString('base64url');

const authError = (
  code: string,
  message: string,
  status?: number,
): ProviderErrorObject =>
  new ProviderErrorObject({
    provider: 'openai',
    code,
    message,
    status,
    retryable: status === 429 || (status !== undefined && status >= 500),
  });
