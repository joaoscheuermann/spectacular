import assert from 'node:assert/strict';
import test from 'node:test';

import { parseEnv } from '../src/lib/env.js';

test('parses dotenv values while allowing later shell overlays', () => {
  assert.deepEqual(parseEnv('A=from-file\nexport B="quoted"\nC=plain'), {
    A: 'from-file',
    B: 'quoted',
    C: 'plain',
  });
});
