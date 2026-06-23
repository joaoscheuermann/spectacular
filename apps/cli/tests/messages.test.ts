import assert from 'node:assert/strict';
import test from 'node:test';

import {
  createFollowupMessageParams,
  createInitialMessageParams,
} from '../src/lib/messages.js';

test('creates initial A2A message params with branch and Codex defaults', () => {
  const params = createInitialMessageParams(
    {
      GITHUB_TOKEN: 'github-token',
      CODEX_AUTHORIZATION: 'codex-token',
    },
    {
      contextId: 'context-1',
      prompt: 'Build the CLI',
      repo: 'https://github.com/example/repo',
      branch: 'feature/doric-cli',
    },
  );

  assert.equal(params.message.contextId, 'context-1');
  assert.deepEqual(params.message.parts, [
    { kind: 'text', text: 'Build the CLI' },
  ]);
  assert.deepEqual(params.message.metadata?.['configuration'], {
    github: {
      repo: {
        url: 'https://github.com/example/repo',
        branch: 'feature/doric-cli',
      },
      token: 'github-token',
    },
    providers: [{ id: 'codex', type: 'codex', token: 'codex-token' }],
    models: [{ id: 'default', provider: 'codex', model: 'gpt-5.5' }],
    tasks: [{ id: 'coding', model: 'default' }],
  });
});

test('creates follow-up messages on the same context and latest task', () => {
  const params = createFollowupMessageParams({
    contextId: 'context-1',
    taskId: 'task-1',
    text: 'Answer',
    blocking: false,
  });

  assert.equal(params.message.contextId, 'context-1');
  assert.equal(params.message.taskId, 'task-1');
  assert.deepEqual(params.message.parts, [{ kind: 'text', text: 'Answer' }]);
  assert.deepEqual(params.configuration, { blocking: false });
});
