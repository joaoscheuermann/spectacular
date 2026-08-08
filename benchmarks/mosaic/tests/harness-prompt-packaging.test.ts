import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import test from 'node:test';
import { promisify } from 'node:util';

const executeFile = promisify(execFile);

test('built conformance loads packaged frozen Markdown prompts', async () => {
  const result = await executeFile(process.execPath, [
    'benchmarks/mosaic/dist/src/cli.js',
    'conformance',
  ]);
  const output = JSON.parse(result.stdout) as {
    readonly ok: boolean;
    readonly result: {
      readonly valid: boolean;
      readonly issues: readonly unknown[];
    };
  };
  assert.equal(output.ok, true);
  assert.equal(output.result.valid, true);
  assert.deepEqual(output.result.issues, []);
});
