import assert from 'node:assert/strict';
import test from 'node:test';

import { execution } from '../index.js';

test('finishes without executing workflow work', async () => {
  const action = await execution(
    { graphs: [] },
    {} as never,
    {
      finish: () => ({ type: 'finish' as const, value: undefined }),
    } as never,
  );

  assert.deepEqual(action, { type: 'finish', value: undefined });
});
