import assert from 'node:assert/strict';
import test from 'node:test';

import { run } from '../src/index.js';
import type { CommandDependencies } from '../src/lib/commands.js';

test('prints help without rejecting or forcing process exit', async () => {
  const stdout: string[] = [];
  const stderr: string[] = [];

  await run(['node', 'doric', '--help'], {
    fetch: async () => {
      throw new Error('Help should not fetch.');
    },
    io: {
      stdout: {
        write(text) {
          stdout.push(text);
        },
      },
      stderr: {
        write(text) {
          stderr.push(text);
        },
      },
    },
    prompt: async () => ({}),
  } satisfies CommandDependencies);

  assert.match(stdout.join(''), /Usage: doric/u);
  assert.deepEqual(stderr, []);
});
