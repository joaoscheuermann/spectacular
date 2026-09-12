import assert from 'node:assert/strict';

import type { LlmProvider, ProviderFinished, ProviderRequest } from 'llms';

const terminalDescription =
  'Submit the final structured output and end the agent run.';

export const mosaicProviders = (provider: LlmProvider) => ({
  planning: provider,
  revision: provider,
  execution: provider,
  reranker: provider,
});

export const terminalTool = (request: ProviderRequest<unknown>) => {
  const tool = request.tools?.find(
    ({ description }) => description === terminalDescription,
  );

  assert.ok(tool);

  assert.equal(tool.name, 'submit_structured_output');

  return tool;
};

export const terminalFinish = (
  request: ProviderRequest<unknown>,
  value: unknown,
  id = 'call-structured-output',
): ProviderFinished<unknown> => {
  const tool = terminalTool(request);
  const argumentsJson = JSON.stringify(value);

  assert.notEqual(argumentsJson, undefined);

  return {
    text: '',
    finishReason: 'tool_calls',
    toolCalls: [
      {
        id,
        name: tool.name,
        arguments: argumentsJson,
      },
    ],
  };
};

export const userContent = (request: ProviderRequest<unknown>): string => {
  const content = request.messages.find(({ role }) => role === 'user')?.content;

  if (typeof content !== 'string') {
    assert.fail('Expected one text user message.');
  }

  return content;
};
