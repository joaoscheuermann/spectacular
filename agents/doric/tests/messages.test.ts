import assert from 'node:assert/strict';
import test from 'node:test';

import {
  checkingGitMessage,
  checkingSessionMessage,
  cloningRepositoryMessage,
  creatingSandboxMessage,
  installingGitMessage,
  sessionReadyMessage,
  taskCreatedMessage,
  updatingSessionConfigMessage,
  usingExistingSessionMessage,
} from '../src/lib/messages/index.js';

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

test('creates a task event for the first progress update', () => {
  const task = taskCreatedMessage('task-1', 'context-1');

  assert.equal(task.kind, 'task');
  assert.equal(task.id, 'task-1');
  assert.equal(task.contextId, 'context-1');
  assert.equal(task.status.state, 'working');
  assert.match(task.status.message?.messageId ?? '', UUID_PATTERN);
  assert.equal(task.status.message?.role, 'agent');
  assert.equal(task.status.message?.taskId, 'task-1');
  assert.equal(task.status.message?.contextId, 'context-1');
  assert.deepEqual(task.status.message?.parts, [
    { kind: 'text', text: 'Created task task-1.' },
  ]);
  assert.deepEqual(task.history, []);
});

test('creates non-final task status updates for session setup progress', () => {
  const updates = [
    [checkingSessionMessage, 'Checking for an existing session.'],
    [updatingSessionConfigMessage, 'Updating the session configuration.'],
    [usingExistingSessionMessage, 'Using the existing session.'],
    [creatingSandboxMessage, 'Creating a sandbox for the repository.'],
    [checkingGitMessage, 'Checking Git inside the sandbox.'],
    [installingGitMessage, 'Installing Git and certificates in the sandbox.'],
    [cloningRepositoryMessage, 'Cloning the configured repository.'],
    [sessionReadyMessage, 'Repository session is ready.'],
  ] as const;

  for (const [createUpdate, text] of updates) {
    const update = createUpdate('task-1', 'context-1');

    assert.equal(update.kind, 'status-update');
    assert.equal(update.taskId, 'task-1');
    assert.equal(update.contextId, 'context-1');
    assert.equal(update.final, false);
    assert.equal(update.status.state, 'working');
    assert.match(update.status.message?.messageId ?? '', UUID_PATTERN);
    assert.equal(update.status.message?.role, 'agent');
    assert.equal(update.status.message?.taskId, 'task-1');
    assert.equal(update.status.message?.contextId, 'context-1');
    assert.deepEqual(update.status.message?.parts, [{ kind: 'text', text }]);
  }
});
