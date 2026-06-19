import assert from 'node:assert/strict';
import test from 'node:test';

import {
  DEFAULT_RPC_PATH,
  HELLO_WORLD_TEXT,
} from '../src/lib/constants/agent.js';
import {
  DEFAULT_SANDBOX_IMAGE,
  GIT_INSTALL_COMMAND,
  GIT_PROBE_COMMAND,
} from '../src/lib/constants/sandbox.js';
import {
  DEFAULT_HOST,
  DEFAULT_PORT,
  MAX_PORT,
  PORT_PATTERN,
} from '../src/lib/constants/server.js';

test('exports the default agent constants', () => {
  assert.equal(HELLO_WORLD_TEXT, 'Hello world from Doric.');
  assert.equal(DEFAULT_RPC_PATH, '/rpc');
});

test('exports the default server constants', () => {
  assert.equal(DEFAULT_HOST, '127.0.0.1');
  assert.equal(DEFAULT_PORT, 4123);
  assert.equal(MAX_PORT, 65_535);
  assert.equal(PORT_PATTERN.test('4123'), true);
  assert.equal(PORT_PATTERN.test('4.123'), false);
});

test('exports the default sandbox constants', () => {
  assert.equal(DEFAULT_SANDBOX_IMAGE, 'node:slim');
  assert.deepEqual(GIT_PROBE_COMMAND, [
    'sh',
    '-lc',
    'command -v git >/dev/null 2>&1',
  ]);
  assert.deepEqual(GIT_INSTALL_COMMAND, [
    'sh',
    '-lc',
    'apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*',
  ]);
});
