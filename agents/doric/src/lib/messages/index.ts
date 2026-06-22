import { randomUUID } from 'node:crypto';

import type { Task, TaskStatusUpdateEvent } from '@a2a-js/sdk';

export const taskCreatedMessage = (
  taskId: string,
  contextId: string,
): Task => ({
  kind: 'task',
  id: taskId,
  contextId,
  status: {
    state: 'working',
    message: {
      kind: 'message',
      messageId: randomUUID(),
      role: 'agent',
      parts: [{ kind: 'text', text: `Created task ${taskId}.` }],
      taskId,
      contextId,
    },
    timestamp: new Date().toISOString(),
  },
  history: [],
});

export const checkingSessionMessage = (taskId: string, contextId: string) =>
  statusUpdate(taskId, contextId, 'Checking for an existing session.');

export const updatingSessionConfigMessage = (
  taskId: string,
  contextId: string,
) => statusUpdate(taskId, contextId, 'Updating the session configuration.');

export const usingExistingSessionMessage = (
  taskId: string,
  contextId: string,
) => statusUpdate(taskId, contextId, 'Using the existing session.');

export const creatingSandboxMessage = (taskId: string, contextId: string) =>
  statusUpdate(taskId, contextId, 'Creating a sandbox for the repository.');

export const checkingGitMessage = (taskId: string, contextId: string) =>
  statusUpdate(taskId, contextId, 'Checking Git inside the sandbox.');

export const installingGitMessage = (taskId: string, contextId: string) =>
  statusUpdate(
    taskId,
    contextId,
    'Installing Git and certificates in the sandbox.',
  );

export const cloningRepositoryMessage = (taskId: string, contextId: string) =>
  statusUpdate(taskId, contextId, 'Cloning the configured repository.');

export const sessionReadyMessage = (taskId: string, contextId: string) =>
  statusUpdate(taskId, contextId, 'Repository session is ready.');

const statusUpdate = (
  taskId: string,
  contextId: string,
  text: string,
): TaskStatusUpdateEvent => ({
  kind: 'status-update',
  taskId,
  contextId,
  final: false,
  status: {
    state: 'working',
    message: {
      kind: 'message',
      messageId: randomUUID(),
      role: 'agent',
      parts: [{ kind: 'text', text }],
      taskId,
      contextId,
    },
    timestamp: new Date().toISOString(),
  },
});
