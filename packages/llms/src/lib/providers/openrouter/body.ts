import type { ProviderRequest } from '../../types/provider.js';
import {
  messageText,
  requireRequestInput,
  structuredJsonSchema,
} from '../common.js';

const structuredOutputName = 'structured_output';

export const openRouterBody = (
  request: ProviderRequest<unknown>,
  stream: boolean,
): Record<string, unknown> => {
  requireRequestInput('openrouter', request);
  const schema = structuredJsonSchema('openrouter', request.schema);

  return prune({
    model: request.model,
    messages: request.messages.map((message) =>
      prune({
        role: message.role,
        content: messageText(message),
        tool_call_id: message.toolCallId,
        tool_calls: message.toolCalls?.map((call) => ({
          id: call.id,
          type: 'function',
          function: { name: call.name, arguments: call.arguments },
        })),
        name: message.name,
      }),
    ),
    tools: request.tools?.map((tool) => ({
      type: 'function',
      function: prune({
        name: tool.name,
        description: tool.description,
        parameters: tool.inputSchema,
      }),
    })),
    temperature: request.temperature,
    max_tokens: request.maxOutputTokens,
    reasoning: reasoningRequest(request),
    response_format:
      request.schema === undefined
        ? undefined
        : {
            type: 'json_schema',
            json_schema: {
              name: structuredOutputName,
              strict: true,
              schema,
            },
          },
    stream,
    stream_options:
      request.flags?.includeUsage === false
        ? undefined
        : { include_usage: true },
  });
};

const reasoningRequest = (
  request: ProviderRequest<unknown>,
): Record<string, unknown> | undefined => {
  const value = request.flags?.reasoning;

  if (value === undefined || value === false) {
    return undefined;
  }

  return value === true
    ? {}
    : prune({ effort: value.effort, summary: value.summary });
};

const prune = (value: Record<string, unknown>): Record<string, unknown> =>
  Object.fromEntries(
    Object.entries(value).filter(([, child]) => child !== undefined),
  );
