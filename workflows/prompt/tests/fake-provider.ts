import assert from 'node:assert/strict';

import type {
  LlmProvider,
  Model,
  ProviderFinished,
  ProviderRequest,
  ProviderStreamEvent,
} from 'llms';

import type { PromptWorkflowOptions } from '../src/index.js';
import { createFakeSandbox, WORKSPACE_ROOT } from './fake-sandbox.js';

type ProviderFake = {
  readonly provider: LlmProvider;
  readonly requests: ProviderRequest<unknown>[];
};

type FakeResponse = unknown | ProviderFinished<unknown>;

export const responsesWithQuestions = (): readonly unknown[] => [
  exploration(),
  requestUnderstanding(),
  intent(),
  {
    questions: [
      {
        question:
          'Which exact requirement fields should downstream agents use?',
        impact: 'Requirement extraction could choose an incompatible schema.',
        recommendation: 'Confirm the public artifact schema first.',
      },
      {
        question: 'Should any old state-machine behavior be preserved?',
        recommendation:
          'Keep the split-pipeline contract unless told otherwise.',
      },
    ],
  },
];

export const responsesWithoutQuestions = (): readonly unknown[] => [
  exploration(),
  requestUnderstanding(),
  intent(),
  { questions: [] },
  {
    requirements: [
      'Preserve the initial request for downstream workflow steps.',
    ],
  },
  {
    requirements: [
      'Run each extraction pass with a fresh agent and message storage.',
    ],
  },
];

export const exploration = () => ({
  summary: 'The workspace contains a prompt workflow package.',
  facts: [
    {
      fact: 'The prompt workflow source is under workflows/prompt.',
      evidencePaths: ['workflows/prompt/src/lib/prompt.ts'],
    },
  ],
});

export const requestUnderstanding = () => ({
  summary: 'The user asked to refactor workflow-prompt into explicit passes.',
});

export const intent = () => ({
  goal: 'Refactor workflow-prompt.',
  scope: 'workflows/prompt',
});

export const workflowOptions = (
  provider: LlmProvider,
  localRoot: string,
): PromptWorkflowOptions => ({
  provider,
  model: 'fake-model',
  workspaceRoot: WORKSPACE_ROOT,
  sandbox: createFakeSandbox(localRoot),
});

export const createProvider = (
  responses: readonly FakeResponse[],
): ProviderFake => {
  const requests: ProviderRequest<unknown>[] = [];
  let next = 0;

  return {
    requests,
    provider: {
      metadata: {
        id: 'fake',
        name: 'Fake',
        baseUrl: 'https://fake.invalid',
      },
      capabilities: {
        streaming: true,
        tools: true,
        reasoning: false,
        modelListing: false,
        oauth: false,
        serviceTier: false,
        structuredOutputs: true,
      },
      complete: async <Output = unknown>(
        request: ProviderRequest<Output>,
      ): Promise<ProviderFinished<Output>> => {
        const index = next;
        const value = responses[index];

        if (value === undefined) {
          throw new Error(`Missing fake provider response ${index}.`);
        }

        next += 1;
        requests.push(request as ProviderRequest<unknown>);

        if (isFinished(value)) {
          return value as ProviderFinished<Output>;
        }

        if (request.schema === undefined) {
          throw new Error(`Missing fake provider schema ${index}.`);
        }

        const parsed = request.schema.safeParse(value);

        if (!parsed.success) {
          throw new Error(
            `Fake provider response ${index} failed schema validation.`,
          );
        }

        return {
          text: '',
          finishReason: 'stop',
          toolCalls: [],
          structured: parsed.data as Output,
        } as ProviderFinished<Output>;
      },
      stream: async function* <Output = unknown>(): AsyncIterable<
        ProviderStreamEvent<Output>
      > {},
      models: async (): Promise<readonly Model[]> => [{ id: 'fake-model' }],
      validateModel: async (model): Promise<Model> => ({ id: model }),
    },
  };
};

export const finishWithToolCalls = (
  toolCalls: readonly ReturnType<typeof toolCall>[],
): ProviderFinished<unknown> => ({
  text: 'Need exploration tools.',
  finishReason: 'tool_calls',
  toolCalls,
});

export const toolCall = (
  name: string,
  payload: Record<string, unknown>,
  id: string,
) => ({
  id,
  name,
  arguments: JSON.stringify(payload),
});

export const createUnstructuredProvider = (text: string): ProviderFake => {
  const requests: ProviderRequest<unknown>[] = [];

  return {
    requests,
    provider: {
      metadata: {
        id: 'fake',
        name: 'Fake',
        baseUrl: 'https://fake.invalid',
      },
      capabilities: {
        streaming: true,
        tools: true,
        reasoning: false,
        modelListing: false,
        oauth: false,
        serviceTier: false,
        structuredOutputs: false,
      },
      complete: async <Output = unknown>(
        request: ProviderRequest<Output>,
      ): Promise<ProviderFinished<Output>> => {
        requests.push(request as ProviderRequest<unknown>);

        return {
          text,
          finishReason: 'stop',
          toolCalls: [],
        };
      },
      stream: async function* <Output = unknown>(): AsyncIterable<
        ProviderStreamEvent<Output>
      > {},
      models: async (): Promise<readonly Model[]> => [{ id: 'fake-model' }],
      validateModel: async (model): Promise<Model> => ({ id: model }),
    },
  };
};

export const messageText = (provider: ProviderFake): string =>
  provider.requests
    .flatMap((request) => request.messages)
    .map((message) =>
      typeof message.content === 'string'
        ? message.content
        : JSON.stringify(message.content ?? ''),
    )
    .join('\n');

export const toolText = (provider: ProviderFake, id: string): string => {
  const value = provider.requests
    .flatMap((request) => request.messages)
    .find(
      (message) => message.role === 'tool' && message.toolCallId === id,
    )?.content;

  if (typeof value !== 'string') {
    assert.fail(`Expected tool message text for ${id}.`);
  }

  return value;
};

const isFinished = (value: unknown): value is ProviderFinished<unknown> => {
  if (typeof value !== 'object' || value === null) {
    return false;
  }

  const candidate = value as Partial<ProviderFinished<unknown>>;

  return (
    typeof candidate.text === 'string' &&
    typeof candidate.finishReason === 'string' &&
    Array.isArray(candidate.toolCalls)
  );
};
