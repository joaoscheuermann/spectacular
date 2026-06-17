import assert from 'node:assert/strict';
import test from 'node:test';

import {
  OAuthErrorObject,
  createLocalCallbackServer,
  openBrowser,
  type BrowserSpawn,
} from '../src/index.js';

test('parses local OAuth callback paths and query parameters', async () => {
  const server = await createLocalCallbackServer({ path: '/oauth/callback' });

  try {
    const waiting = server.waitForCallback('state-123');
    const response = await fetch(
      `${server.redirectUri}?code=code-123&state=state-123`,
    );
    const callback = await waiting;

    assert.equal(response.status, 200);
    assert.equal(callback.code, 'code-123');
    assert.equal(callback.state, 'state-123');
  } finally {
    await server.close();
  }
});

test('rejects pending local OAuth callbacks when the server closes', async () => {
  const server = await createLocalCallbackServer();
  const waiting = server.waitForCallback('state-123');

  await server.close();

  await assert.rejects(waiting, hasCode('oauth_callback_server_closed'));
});

test('rejects pending local OAuth callbacks when the signal aborts', async () => {
  const server = await createLocalCallbackServer();
  const controller = new AbortController();
  const waiting = server.waitForCallback('state-123', controller.signal);

  controller.abort();

  try {
    await assert.rejects(waiting, hasCode('oauth_callback_aborted'));
  } finally {
    await server.close();
  }
});

test('selects browser opener commands by platform', async () => {
  const url = 'https://example.test/callback?code=abc&state=xyz';
  const calls: { command: string; args: readonly string[] }[] = [];
  const spawn: BrowserSpawn = (command, args) => {
    calls.push({ command, args });

    return {
      unref() {
        return undefined;
      },
    };
  };

  await openBrowser(url, { platform: 'win32', spawn });
  await openBrowser(url, { platform: 'darwin', spawn });
  await openBrowser(url, { platform: 'linux', spawn });

  assert.deepEqual(calls, [
    {
      command: 'rundll32',
      args: ['url.dll,FileProtocolHandler', url],
    },
    { command: 'open', args: [url] },
    { command: 'xdg-open', args: [url] },
  ]);
});

const hasCode = (code: string) => (error: unknown): boolean =>
  error instanceof OAuthErrorObject && error.data.code === code;
