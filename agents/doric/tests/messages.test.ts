import assert from 'node:assert/strict';
import test from 'node:test';

import {
  checkingGitMessage,
  checkingSessionMessage,
  cloningRepositoryMessage,
  creatingSandboxMessage,
  installingGitMessage,
  promptArtifactMessage,
  promptCompletedMessage,
  promptInputRequiredMessage,
  runningPromptWorkflowMessage,
  sessionReadyMessage,
  taskCanceledMessage,
  taskCreatedMessage,
  updatingSessionConfigMessage,
  usingExistingSessionMessage,
} from '../src/lib/messages/index.js';
import { createPromptArtifact } from 'workflow-prompt';

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
    [runningPromptWorkflowMessage, 'Running workflow-prompt.'],
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

test('creates prompt artifact update events', () => {
  const artifact = createPromptArtifact('Build the agent');
  const update = promptArtifactMessage(
    'task-1',
    'context-1',
    artifact,
    '# Prompt',
  );

  assert.equal(update.kind, 'artifact-update');
  assert.equal(update.taskId, 'task-1');
  assert.equal(update.contextId, 'context-1');
  assert.equal(update.lastChunk, true);
  assert.equal(update.artifact.artifactId, 'PROMPT');
  assert.equal(update.artifact.name, 'PROMPT.md');
  assert.deepEqual(update.artifact.parts, [{ kind: 'text', text: '# Prompt' }]);
  assert.deepEqual(update.artifact.metadata, { mime: 'text/markdown' });
});

test('creates final prompt workflow status updates', () => {
  const inputRequired = promptInputRequiredMessage('task-1', 'context-1', [
    {
      question: 'Which model should be used?',
      impact: 'The run cannot continue.',
      recommendation: 'Use the coding task model.',
      options: [
        'Use the coding task model.',
        'Use the planning model for this run.',
        'Ask the caller to provide a model override.',
      ],
    },
    {
      question: 'Should the prompt include validation?',
      recommendation: 'Include the configured checks.',
      options: [
        'Include the configured checks.',
        'Skip validation and only draft the prompt.',
        'Ask the caller for the exact validation commands.',
      ],
    },
  ]);
  const completed = promptCompletedMessage('task-1', 'context-1');

  assert.equal(inputRequired.kind, 'status-update');
  assert.equal(inputRequired.final, true);
  assert.equal(inputRequired.status.state, 'input-required');
  assert.deepEqual(inputRequired.status.message?.parts, [
    {
      kind: 'data',
      data: {
        kind: 'prompt-open-questions',
        questions: [
          {
            id: 'open-question-1',
            title:
              'Which model should be used? Recommendation: Use the coding task model.',
            question: 'Which model should be used?',
            recommendation: 'Use the coding task model.',
            impact: 'The run cannot continue.',
            options: [
              {
                id: 'option-1',
                title: 'Use the coding task model.',
                value: 'Use the coding task model.',
              },
              {
                id: 'option-2',
                title: 'Use the planning model for this run.',
                value: 'Use the planning model for this run.',
              },
              {
                id: 'option-3',
                title: 'Ask the caller to provide a model override.',
                value: 'Ask the caller to provide a model override.',
              },
            ],
          },
          {
            id: 'open-question-2',
            title:
              'Should the prompt include validation? Recommendation: Include the configured checks.',
            question: 'Should the prompt include validation?',
            recommendation: 'Include the configured checks.',
            options: [
              {
                id: 'option-1',
                title: 'Include the configured checks.',
                value: 'Include the configured checks.',
              },
              {
                id: 'option-2',
                title: 'Skip validation and only draft the prompt.',
                value: 'Skip validation and only draft the prompt.',
              },
              {
                id: 'option-3',
                title: 'Ask the caller for the exact validation commands.',
                value: 'Ask the caller for the exact validation commands.',
              },
            ],
          },
        ],
      },
    },
  ]);

  assert.equal(completed.kind, 'status-update');
  assert.equal(completed.final, true);
  assert.equal(completed.status.state, 'completed');
  assert.deepEqual(completed.status.message?.parts, [
    {
      kind: 'text',
      text: 'Prompt workflow completed. PROMPT artifact is ready.',
    },
  ]);
});

test('creates final task cancellation status updates', () => {
  const canceled = taskCanceledMessage('task-1', 'context-1');

  assert.equal(canceled.kind, 'status-update');
  assert.equal(canceled.taskId, 'task-1');
  assert.equal(canceled.contextId, 'context-1');
  assert.equal(canceled.final, true);
  assert.equal(canceled.status.state, 'canceled');
  assert.deepEqual(canceled.status.message?.parts, [
    { kind: 'text', text: 'Task cancellation requested by user.' },
  ]);
});

test('throws before creating input-required event when an open question has fewer than three options', () => {
  const malformedQuestions = [
    {
      question: 'Which model should be used?',
      recommendation: 'Use the coding task model.',
      options: ['Use the coding task model.', 'Use the planning model.'],
    },
  ] as unknown as Parameters<typeof promptInputRequiredMessage>[2];

  assert.throws(
    () =>
      promptInputRequiredMessage('task-1', 'context-1', malformedQuestions),
    /Prompt open question 1 must include at least 3 options before emitting an input-required event/u,
  );
});
