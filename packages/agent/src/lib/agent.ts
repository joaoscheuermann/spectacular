import type {
  JsonValue,
  ProviderFinished,
  ProviderMessage,
  ProviderRequest,
} from 'llms';

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

  const buildRequest = <Output = JsonValue>(
    runOptions: AgentRunOptions<Output>,
  ): ProviderRequest<Output> => {
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
      ...(options.effort !== undefined ? { effort: options.effort } : {}),
      ...(options.flags !== undefined ? { flags: options.flags } : {}),
      ...(runOptions.schema !== undefined ? { schema: runOptions.schema } : {}),
      ...(runOptions.signal !== undefined ? { signal: runOptions.signal } : {}),
    };
  };

  const pushToolResult = (callId: string, content: string): void => {
    options.messages.push({
      role: 'tool',
      toolCallId: callId,
      content,
    });
  };

  const storeAssistant = (finish: ProviderFinished<unknown>): void => {
    options.messages.push({
      role: 'assistant',
      content:
        finish.text.length > 0 ? finish.text : (finish.refusal ?? finish.text),
      ...(finish.toolCalls.length > 0 ? { toolCalls: finish.toolCalls } : {}),
    });
  };

  const runTools = async (finish: ProviderFinished<unknown>): Promise<void> => {
    for (const call of options.tools.calls(finish)) {
      const result = await options.tools.execute(call);
      pushToolResult(call.id, serializeToolResult(result));
    }
  };

  return {
    complete: async <Output = JsonValue>(
      input: string,
      runOptions: AgentRunOptions<Output> = {},
    ): Promise<AgentResponse<Output>> => {
      acquire();

      try {
        options.messages.push({ role: 'user', content: input });

        while (true) {
          const finish = await options.provider.complete(
            buildRequest(runOptions),
          );
          storeAssistant(finish);

          if (finish.toolCalls.length === 0) {
            return responseFromFinish(finish);
          }

          await runTools(finish);
        }
      } finally {
        release();
      }
    },

    stream: async function* <Output = JsonValue>(
      input: string,
      runOptions: AgentRunOptions<Output> = {},
    ) {
      acquire();

      try {
        options.messages.push({ role: 'user', content: input });
        yield {
          type: 'agent.started',
          model: options.model,
          input,
        } as const;

        while (true) {
          let finish: ProviderFinished<Output> | undefined;

          for await (const event of options.provider.stream(
            buildRequest(runOptions),
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

          storeAssistant(finish);

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

const responseFromFinish = <Output = JsonValue>(
  finish: ProviderFinished<Output>,
): AgentResponse<Output> => ({
  text: finish.text,
  finishReason: finish.finishReason,
  usage: finish.usage,
  reasoning: finish.reasoning,
  refusal: finish.refusal,
  structured: finish.structured,
  finish,
});
