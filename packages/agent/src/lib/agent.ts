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
import {
  createStructuredOutputTool,
  missingStructuredOutput,
  parseStructuredOutputTool,
  structuredOutputInstruction,
  type StructuredOutputTool,
} from './utils/structured-output.js';

/** Creates an embeddable agent runtime from injected provider, tool, and message boundaries. */
export const createAgent = (options: AgentOptions): Agent => {
  /** One agent owns one mutable message history and therefore runs serially. */
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
    outputTool?: StructuredOutputTool,
  ): ProviderRequest<Output> => {
    /** Executable definitions come from the caller-owned tool storage. */
    const definitions = options.tools.definitions();

    /**
     * Keep the authored system prompt first. Tool-enabled structured runs add
     * one generated instruction that tells the model how to end the loop.
     */
    const system: readonly ProviderMessage[] = [
      ...(options.system.length > 0
        ? [{ role: 'system' as const, content: options.system }]
        : []),
      ...(outputTool === undefined
        ? []
        : [
            {
              role: 'system' as const,
              content: structuredOutputInstruction(outputTool.name),
            },
          ]),
    ];
    /** The terminal output definition is visible to the model but not executable. */
    const tools =
      outputTool === undefined
        ? definitions
        : [...definitions, outputTool.definition];

    /**
     * Native provider schemas are used only when no terminal tool is needed.
     * This avoids sending executable tools and provider-native output format
     * controls together, which is unsupported by some providers.
     */
    return {
      model: options.model,
      messages: [...system, ...options.messages.list()],
      ...(tools.length > 0 ? { tools } : {}),
      ...(options.temperature !== undefined
        ? { temperature: options.temperature }
        : {}),
      ...(options.maxOutputTokens !== undefined
        ? { maxOutputTokens: options.maxOutputTokens }
        : {}),
      ...(options.effort !== undefined ? { effort: options.effort } : {}),
      ...(options.flags !== undefined ? { flags: options.flags } : {}),
      ...(runOptions.schema !== undefined && outputTool === undefined
        ? { schema: runOptions.schema }
        : {}),
      ...(runOptions.signal !== undefined ? { signal: runOptions.signal } : {}),
    };
  };

  const outputTool = <Output>(
    runOptions: AgentRunOptions<Output>,
  ): StructuredOutputTool | undefined => {
    const definitions = options.tools.definitions();

    /** Plain structured calls stay provider-native; mixed runs need a terminal tool. */
    return runOptions.schema === undefined || definitions.length === 0
      ? undefined
      : createStructuredOutputTool(
          options.provider.metadata.id,
          runOptions.schema,
          definitions,
        );
  };

  const pushToolResult = (callId: string, content: string): void => {
    /** Tool results retain the provider call ID for the next model turn. */
    options.messages.push({
      role: 'tool',
      toolCallId: callId,
      content,
    });
  };

  const storeAssistant = (finish: ProviderFinished<unknown>): void => {
    /** Persist both text and tool-call requests in the isolated conversation. */
    options.messages.push({
      role: 'assistant',
      content:
        finish.text.length > 0 ? finish.text : (finish.refusal ?? finish.text),
      ...(finish.toolCalls.length > 0 ? { toolCalls: finish.toolCalls } : {}),
    });
  };

  const runTools = async (finish: ProviderFinished<unknown>): Promise<void> => {
    /** Execute provider-requested tools sequentially in provider order. */
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
        /** The caller input starts the run-local conversation. */
        options.messages.push({ role: 'user', content: input });

        /** This value remains stable across every provider turn in the run. */
        const terminal = outputTool(runOptions);

        while (true) {
          /** Ask the provider for either executable calls or the terminal result. */
          const finish = await options.provider.complete(
            buildRequest(runOptions, terminal),
          );

          /** Intercept and validate the reserved terminal call before normal tools. */
          const structured =
            terminal === undefined
              ? undefined
              : parseStructuredOutputTool<Output>(finish, terminal);

          if (structured !== undefined) {
            /** Store the normalized tool-free finish as the final assistant message. */
            storeAssistant(structured);
            return responseFromFinish(structured);
          }

          /** Ordinary assistant turns remain available to later tool iterations. */
          storeAssistant(finish);

          if (finish.toolCalls.length === 0) {
            /** A tool-enabled structured run may end only through its terminal tool. */
            if (terminal !== undefined) throw missingStructuredOutput();

            return responseFromFinish(finish);
          }

          /** Tool results are appended before the loop requests the next turn. */
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
        /** Streaming uses the same message and terminal-tool contract as complete. */
        options.messages.push({ role: 'user', content: input });
        const terminal = outputTool(runOptions);
        yield {
          type: 'agent.started',
          model: options.model,
          input,
        } as const;

        while (true) {
          let finish: ProviderFinished<Output> | undefined;

          for await (const event of options.provider.stream(
            buildRequest(runOptions, terminal),
          )) {
            if (event.type === 'response.finished') {
              /** Normalize a terminal call before exposing the finished event. */
              const structured =
                terminal === undefined
                  ? undefined
                  : parseStructuredOutputTool<Output>(event.finish, terminal);

              if (
                terminal !== undefined &&
                structured === undefined &&
                event.finish.toolCalls.length === 0
              ) {
                throw missingStructuredOutput();
              }

              finish = structured ?? event.finish;
              yield { ...event, finish };
              continue;
            }

            yield event;
          }

          if (finish === undefined) {
            throw new AgentErrorObject({
              code: 'missing_provider_finish',
              message:
                'Provider stream ended without a response.finished event.',
            });
          }

          storeAssistant(finish);

          /** A normalized terminal finish has no calls and completes the stream. */
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
            /** Streaming exposes tool lifecycle events around the same execution path. */
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
