import assert from 'node:assert/strict';
import test from 'node:test';

import { writeSessionList } from '../src/lib/render.js';

test('prints session rows as context, state, and prompt', () => {
  const output: string[] = [];

  writeSessionList(
    {
      stdout: {
        write(text) {
          output.push(text);
        },
      },
    },
    [
      {
        contextId: 'context-1',
        state: 'input-required',
        prompt: 'Build\nCLI',
      },
    ],
  );

  assert.deepEqual(output, ['context-1 input-required Build CLI\n']);
});
