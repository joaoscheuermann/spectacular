import assert from 'node:assert/strict';
import test from 'node:test';

import { createProcessRunner } from '../src/process.js';

test('mirrors command output live while retaining the final result', async () => {
  const chunks: string[] = [];

  const runner = createProcessRunner(process.env, {
    write: (value) => (chunks.push(String(value)), true),
  });

  const result = await runner({
    file: process.execPath,
    args: ['-e', 'process.stdout.write("out"); process.stderr.write("err")'],
    cwd: process.cwd(),
  });

  assert.equal(result.code, 0);

  assert.equal(result.stdout, 'out');

  assert.equal(result.stderr, 'err');

  assert.match(chunks.join(''), /out/);

  assert.match(chunks.join(''), /err/);
});
