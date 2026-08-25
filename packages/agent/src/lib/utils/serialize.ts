import { AgentErrorObject } from '../classes/agent-error.js';

export const serializeToolResult = (result: unknown): string => {
  if (typeof result === 'string') {
    return result;
  }

  if (result === undefined) {
    return '';
  }

  try {
    const content = JSON.stringify(result);

    if (content !== undefined) {
      return content;
    }
  } catch (cause) {
    throw new AgentErrorObject(
      {
        code: 'tool_result_serialization_failed',
        message: 'Tool result could not be serialized to JSON.',
      },
      { cause },
    );
  }

  throw new AgentErrorObject({
    code: 'tool_result_serialization_failed',
    message: 'Tool result could not be serialized to JSON.',
  });
};
