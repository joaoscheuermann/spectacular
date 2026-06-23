import { randomUUID } from 'node:crypto';

import type {
  Artifact,
  DataPart,
  Part,
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
  statusUpdateWithParts(
    taskId,
    contextId,
    [openQuestionsPart(questions)],
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
): TaskStatusUpdateEvent =>
  statusUpdateWithParts(
    taskId,
    contextId,
    [{ kind: 'text', text }],
    state,
    final,
  );

const statusUpdateWithParts = (
  taskId: string,
  contextId: string,
  parts: Part[],
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
      parts,
      taskId,
      contextId,
    },
    timestamp: new Date().toISOString(),
  },
});

const openQuestionsPart = (
  questions: readonly PromptOpenQuestion[],
): DataPart => ({
  kind: 'data',
  data: {
    kind: 'prompt-open-questions',
    questions: questions.map(openQuestionData),
  },
});

const openQuestionData = (question: PromptOpenQuestion, index: number) => ({
  id: `open-question-${index + 1}`,
  title: `${question.question} Recommendation: ${question.recommendation}`,
  question: question.question,
  recommendation: question.recommendation,
  ...(question.impact === undefined ? {} : { impact: question.impact }),
  options: [
    {
      id: 'recommendation',
      title: question.recommendation,
      value: question.recommendation,
    },
  ],
});
