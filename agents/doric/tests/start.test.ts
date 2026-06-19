import assert from 'node:assert/strict';
import test from 'node:test';

import {
  DEFAULT_HOST,
  DEFAULT_PORT,
  formatListenMessage,
  resolveListenOptions,
  startServer,
} from '../src/index.js';

test('resolves the local default host and port when environment values are absent', () => {
  assert.deepEqual(resolveListenOptions({}), {
    host: DEFAULT_HOST,
    port: DEFAULT_PORT,
  });
});

test('resolves host and port from environment values when they are present', () => {
  assert.deepEqual(resolveListenOptions({ HOST: '0.0.0.0', PORT: '8080' }), {
    host: '0.0.0.0',
    port: 8080,
  });
});

test('throws a clear error when the environment port is invalid', () => {
  for (const port of ['', 'abc', '1.5', '-1', '65536']) {
    assert.throws(
      () => resolveListenOptions({ PORT: port }),
      /PORT must be an integer from 0 to 65535/u,
    );
  }
});

test('starts the server on an ephemeral port and writes the listening message', async () => {
  const lines: string[] = [];
  const started = await startServer({
    host: DEFAULT_HOST,
    port: 0,
    writeLine: (line) => {
      lines.push(line);
    },
  });

  try {
    assert.equal(started.host, DEFAULT_HOST);
    assert.ok(started.port > 0);
    assert.equal(started.origin, `http://${DEFAULT_HOST}:${started.port}`);
    assert.deepEqual(lines, [formatListenMessage(started.origin)]);

    const response = await fetch(
      `${started.origin}/.well-known/agent-card.json`,
    );

    assert.equal(response.status, 200);
  } finally {
    await started.close();
  }
});
