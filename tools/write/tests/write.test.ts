import { mkdtemp, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { describe, test } from 'node:test';

import { createTool } from '../src/index.js';

describe('write tool', () => {
  test('creates parent directories and returns byte count with diff', async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), 'doric-write-'));

    const result = await createTool({ workspaceRoot: root }).execute({
      path: 'notes/today.txt',
      content: 'hello',
    });

    assert.equal(result.success, true);
    assert.equal(result.bytes_written, 5);
    assert.match(result.diff ?? '', /1 \+hello/);
    assert.equal(
      await readFile(path.join(root, 'notes/today.txt'), 'utf8'),
      'hello',
    );
    await rm(root, { recursive: true, force: true });
  });

  test('rejects empty paths with structured error output', async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), 'doric-write-empty-'));

    const result = await createTool({ workspaceRoot: root }).execute({
      path: '',
      content: 'hello',
    });

    assert.deepEqual(result, {
      success: false,
      bytes_written: 0,
      error: 'Path must not be empty',
    });
    await rm(root, { recursive: true, force: true });
  });
});
