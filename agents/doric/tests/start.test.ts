import assert from 'node:assert/strict';
import test from 'node:test';

import {
  DEFAULT_DORIC_HOST,
  DEFAULT_DORIC_PORT,
  formatDoricListenMessage,
  resolveDoricListenOptions,
  startDoricServer,
} from '../src/index.js';

test('resolves the local default host and port when environment values are absent', () => {
  assert.deepEqual(resolveDoricListenOptions({}), {
    host: DEFAULT_DORIC_HOST,
    port: DEFAULT_DORIC_PORT,
  });
});

test('resolves host and port from environment values when they are present', () => {
  assert.deepEqual(
    resolveDoricListenOptions({ HOST: '0.0.0.0', PORT: '8080' }),
    {
      host: '0.0.0.0',
      port: 8080,
    },
  );
});

test('throws a clear error when the environment port is invalid', () => {
  for (const port of ['', 'abc', '1.5', '-1', '65536']) {
    assert.throws(
      () => resolveDoricListenOptions({ PORT: port }),
      /PORT must be an integer from 0 to 65535/u,
    );
  }
});

test('starts the server on an ephemeral port and writes the listening message', async () => {
  const lines: string[] = [];
  const started = await startDoricServer({
    host: DEFAULT_DORIC_HOST,
    port: 0,
    writeLine: (line) => {
      lines.push(line);
    },
  });

  try {
    assert.equal(started.host, DEFAULT_DORIC_HOST);
    assert.ok(started.port > 0);
    assert.equal(
      started.origin,
      `http://${DEFAULT_DORIC_HOST}:${started.port}`,
    );
    assert.deepEqual(lines, [formatDoricListenMessage(started.origin)]);

    const response = await fetch(
      `${started.origin}/.well-known/agent-card.json`,
    );

    assert.equal(response.status, 200);
  } finally {
    await started.close();
  }
});
