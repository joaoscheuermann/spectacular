import { randomUUID } from 'node:crypto';

import type {
  Artifact,
  Task,
  TaskArtifactUpdateEvent,
  TaskState,
  TaskStatusUpdateEvent,
} from '@a2a-js/sdk';

import type { PromptArtifact, PromptOpenQuestion } from 'workflow-prompt';

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

export const runningPromptWorkflowMessage = (
  taskId: string,
  contextId: string,
) => statusUpdate(taskId, contextId, 'Running workflow-prompt.');

export const promptArtifactMessage = (
  taskId: string,
  contextId: string,
  artifact: PromptArtifact,
  rendered: string,
): TaskArtifactUpdateEvent => ({
  kind: 'artifact-update',
  taskId,
  contextId,
  lastChunk: true,
  artifact: {
    artifactId: artifact.name,
    name: `${artifact.name}.md`,
    description: 'Rendered workflow-prompt artifact.',
    parts: [{ kind: 'text', text: rendered }],
    metadata: { mime: artifact.mime },
  } satisfies Artifact,
});

export const promptInputRequiredMessage = (
  taskId: string,
  contextId: string,
  questions: readonly PromptOpenQuestion[],
) =>
  statusUpdate(
    taskId,
    contextId,
    openQuestionsText(questions),
    'input-required',
    true,
  );

export const promptCompletedMessage = (taskId: string, contextId: string) =>
  statusUpdate(
    taskId,
    contextId,
    'Prompt workflow completed. PROMPT artifact is ready.',
    'completed',
    true,
  );

const statusUpdate = (
  taskId: string,
  contextId: string,
  text: string,
  state: TaskState = 'working',
  final = false,
): TaskStatusUpdateEvent => ({
  kind: 'status-update',
  taskId,
  contextId,
  final,
  status: {
    state,
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

const openQuestionsText = (questions: readonly PromptOpenQuestion[]): string =>
  [
    'I need input before finalizing PROMPT.',
    '',
    ...questions.map(formatOpenQuestion),
  ].join('\n');

const formatOpenQuestion = (
  question: PromptOpenQuestion,
  index: number,
): string =>
  [
    `${index + 1}. ${question.question}`,
    `Recommendation: ${question.recommendation}`,
    ...(question.impact === undefined ? [] : [`Impact: ${question.impact}`]),
  ].join('\n');
