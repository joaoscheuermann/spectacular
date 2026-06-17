import type { ProviderFinished, ProviderMessage, ProviderRequest } from 'llms';

import { AgentErrorObject } from './classes/agent-error.js';
import type {
  Agent,
  AgentOptions,
  AgentResponse,
  AgentRunOptions,
} from './types/agent.js';
import { serializeToolResult } from './utils/serialize.js';

/** Creates an embeddable agent runtime from injected provider, tool, and message boundaries. */
export const createAgent = (options: AgentOptions): Agent => {
  let running = false;

  const acquire = (): void => {
    if (running) {
      throw new AgentErrorObject({
        code: 'concurrent_run',
        message: 'Agent instances do not support concurrent runs.',
      });
    }

    running = true;
  };

  const release = (): void => {
    running = false;
  };

  const buildRequest = (signal?: AbortSignal): ProviderRequest => {
    const system: readonly ProviderMessage[] =
      options.system.length > 0
        ? [{ role: 'system', content: options.system }]
        : [];
    const definitions = options.tools.definitions();

    return {
      model: options.model,
      messages: [...system, ...options.messages.list()],
      ...(definitions.length > 0 ? { tools: definitions } : {}),
      ...(options.temperature !== undefined
        ? { temperature: options.temperature }
        : {}),
      ...(options.maxOutputTokens !== undefined
        ? { maxOutputTokens: options.maxOutputTokens }
        : {}),
      ...(options.flags !== undefined ? { flags: options.flags } : {}),
      ...(signal !== undefined ? { signal } : {}),
    };
  };

  const pushToolResult = (callId: string, content: string): void => {
    options.messages.push({
      role: 'tool',
      toolCallId: callId,
      content,
    });
  };

  const runTools = async (finish: ProviderFinished): Promise<void> => {
    for (const call of options.tools.calls(finish)) {
      const result = await options.tools.execute(call);
      pushToolResult(call.id, serializeToolResult(result));
    }
  };

  return {
    complete: async (
      input: string,
      runOptions: AgentRunOptions = {},
    ): Promise<AgentResponse> => {
      acquire();

      try {
        options.messages.push({ role: 'user', content: input });

        while (true) {
          const finish = await options.provider.complete(
            buildRequest(runOptions.signal),
          );
          options.messages.push(finish);

          if (finish.toolCalls.length === 0) {
            return responseFromFinish(finish);
          }

          await runTools(finish);
        }
      } finally {
        release();
      }
    },

    stream: async function* (input: string, runOptions: AgentRunOptions = {}) {
      acquire();

      try {
        options.messages.push({ role: 'user', content: input });
        yield {
          type: 'agent.started',
          model: options.model,
          input,
        } as const;

        while (true) {
          let finish: ProviderFinished | undefined;

          for await (const event of options.provider.stream(
            buildRequest(runOptions.signal),
          )) {
            yield event;

            if (event.type === 'response.finished') {
              finish = event.finish;
            }
          }

          if (finish === undefined) {
            throw new AgentErrorObject({
              code: 'missing_provider_finish',
              message:
                'Provider stream ended without a response.finished event.',
            });
          }

          options.messages.push(finish);

          const calls = options.tools.calls(finish);

          if (calls.length === 0) {
            const response = responseFromFinish(finish);
            yield {
              type: 'agent.finished',
              response,
            } as const;
            return;
          }

          for (const call of calls) {
            yield {
              type: 'tool.started',
              call,
            } as const;

            let result: unknown;

            try {
              result = await options.tools.execute(call);
            } catch (error) {
              yield {
                type: 'tool.failed',
                call,
                error,
              } as const;
              throw error;
            }

            const content = serializeToolResult(result);
            pushToolResult(call.id, content);

            yield {
              type: 'tool.finished',
              call,
              result,
              content,
            } as const;
          }
        }
      } finally {
        release();
      }
    },
  };
};

const responseFromFinish = (finish: ProviderFinished): AgentResponse => ({
  text: finish.text,
  finishReason: finish.finishReason,
  usage: finish.usage,
  reasoning: finish.reasoning,
  refusal: finish.refusal,
  finish,
});
