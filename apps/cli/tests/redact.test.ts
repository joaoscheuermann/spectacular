import assert from 'node:assert/strict';
import test from 'node:test';

import { createTextRedactor, redact } from '../src/lib/redact.js';

test('redacts token-like keys and values from output payloads', () => {
  const redactor = createTextRedactor({
    GITHUB_TOKEN: 'github-token',
    CODEX_AUTHORIZATION: 'codex-token',
  });

  assert.deepEqual(
    redact(
      {
        token: 'github-token',
        nested: { message: 'use codex-token' },
      },
      redactor,
    ),
    {
      token: '[redacted]',
      nested: { message: 'use [redacted]' },
    },
  );
});
