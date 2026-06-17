import assert from 'node:assert/strict';
import test from 'node:test';

import {
  ProviderErrorObject,
  getOpenAiAuthHeader,
  parseOpenAiTokenClaims,
  refreshOpenAiAuthRecord,
  runOpenAiOAuthCallbackFlow,
  type OpenAiAuthRecord,
  type OpenAiAuthStore,
} from '../src/index.js';
import { fakeTransport, response } from './fakes.js';

test('parses OpenAI OAuth token claims from JWT payloads', () => {
  const token = [
    'header',
    Buffer.from(JSON.stringify({ sub: 'user_1', email: 'a@example.com' })).toString('base64url'),
    'sig',
  ].join('.');

  assert.deepEqual(parseOpenAiTokenClaims(token), {
    sub: 'user_1',
    email: 'a@example.com',
  });
  assert.equal(parseOpenAiTokenClaims('not-a-jwt'), undefined);
});

test('refreshes OpenAI OAuth records before use when expiry is inside skew', async () => {
  const saved: OpenAiAuthRecord[] = [];
  const store = memoryStore({
    accessToken: 'old.token.value',
    refreshToken: 'refresh-token',
    expiresAt: 1_050,
  });
  const transport = fakeTransport({
    responses: [
      response({
        access_token: 'new.token.value',
        refresh_token: 'new-refresh',
        expires_in: 10,
        token_type: 'Bearer',
      }),
    ],
  });

  store.onSave = (record) => saved.push(record);

  const header = await getOpenAiAuthHeader({
    transport,
    authStore: store,
    clientId: 'client',
    clock: () => 1_000,
    refreshSkewMs: 100,
  });

  assert.equal(header, 'Bearer new.token.value');
  assert.equal(saved[0]?.refreshToken, 'new-refresh');
  assert.equal(saved[0]?.expiresAt, 11_000);
});

test('runs OpenAI OAuth PKCE callback flow and stores exchanged token', async () => {
  let opened = '';
  const saved: OpenAiAuthRecord[] = [];
  const store = memoryStore();
  store.onSave = (record) => saved.push(record);
  const transport = fakeTransport({
    responses: [
      response({
        access_token: 'new.token.value',
        refresh_token: 'refresh-token',
        expires_in: 3600,
      }),
    ],
  });

  const record = await runOpenAiOAuthCallbackFlow({
    transport,
    authStore: store,
    clientId: 'client',
    redirectUri: 'http://127.0.0.1/callback',
    browserOpener(url) {
      opened = url;
    },
    callbackServer: {
      async waitForCallback(expectedState) {
        return { code: 'code-123', state: expectedState };
      },
    },
    random: {
      bytes(length) {
        return new Uint8Array(Array.from({ length }, (_, index) => index + 1));
      },
    },
    clock: () => 1_000,
  });

  const url = new URL(opened);
  assert.equal(url.searchParams.get('code_challenge_method'), 'S256');
  assert.equal(url.searchParams.get('client_id'), 'client');
  assert.equal(record.accessToken, 'new.token.value');
  assert.equal(saved.length, 1);
  assert.match(transport.requests[0]?.body ?? '', /code_verifier=/);
});

test('rejects OpenAI OAuth callback state and provider errors', async () => {
  await assert.rejects(
    runOpenAiOAuthCallbackFlow({
      transport: fakeTransport({}),
      authStore: memoryStore(),
      clientId: 'client',
      redirectUri: 'http://127.0.0.1/callback',
      browserOpener() {
        return undefined;
      },
      callbackServer: {
        async waitForCallback() {
          return { code: 'code-123', state: 'wrong' };
        },
      },
      random: {
        bytes(length) {
          return new Uint8Array(length).fill(1);
        },
      },
    }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'oauth_state_mismatch',
  );

  await assert.rejects(
    refreshOpenAiAuthRecord({
      transport: fakeTransport({ responses: [response('bad', 401)] }),
      authStore: memoryStore({
        accessToken: 'old.token.value',
        refreshToken: 'refresh-token',
      }),
    }),
    (error: unknown) =>
      error instanceof ProviderErrorObject &&
      error.data.code === 'oauth_refresh_failed',
  );
});

type MutableStore = OpenAiAuthStore & {
  onSave?: (record: OpenAiAuthRecord) => void;
};

const memoryStore = (initial?: OpenAiAuthRecord): MutableStore => {
  let record = initial;

  return {
    async load() {
      return record;
    },

    async save(next) {
      record = next;
      this.onSave?.(next);
    },
  };
};
