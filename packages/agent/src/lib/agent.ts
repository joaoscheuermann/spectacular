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
  AgentStructuredAttemptEvent,
  AgentToolCallRepairEvent,
} from './types/agent.js';
import { serializeToolResult } from './utils/serialize.js';
import {
  createStructuredOutputTool,
  nextStructuredOutputRepair,
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

  const turnGuard = (maxTurns: number | undefined): (() => void) => {
    if (
      maxTurns !== undefined &&
      (!Number.isSafeInteger(maxTurns) || maxTurns <= 0)
    ) {
      throw new TypeError('Agent maxTurns must be a positive safe integer.');
    }

    let turns = 0;

    return (): void => {
      if (maxTurns !== undefined && turns >= maxTurns) {
        throw new AgentErrorObject({
          code: 'turn_limit_exceeded',
          message: `Agent exceeded its ${maxTurns}-turn limit.`,
        });
      }

      turns += 1;
    };
  };

  const buildRequest = <Output = JsonValue>(
    runOptions: AgentRunOptions<Output>,
    outputTool?: StructuredOutputTool,
    correction?: string,
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
      ...(correction === undefined
        ? []
        : [{ role: 'system' as const, content: correction }]),
    ];
    /** The terminal output definition is visible to the model but not executable. */
    const tools =
      outputTool === undefined
        ? definitions
        : [...definitions, outputTool.definition];
    return {
      model: options.model,
      messages: [...system, ...options.messages.list()],
      ...(tools.length > 0 ? { tools } : {}),
      ...(outputTool === undefined ? {} : { parallelToolCalls: false }),
      ...(options.temperature !== undefined
        ? { temperature: options.temperature }
        : {}),
      ...(options.maxOutputTokens !== undefined
        ? { maxOutputTokens: options.maxOutputTokens }
        : {}),
      ...(options.effort !== undefined ? { effort: options.effort } : {}),
      ...(options.flags !== undefined ? { flags: options.flags } : {}),
      ...(runOptions.signal !== undefined ? { signal: runOptions.signal } : {}),
    };
  };

  const outputTool = <Output>(
    runOptions: AgentRunOptions<Output>,
  ): StructuredOutputTool | undefined => {
    const definitions = options.tools.definitions();

    /** Every structured run terminates through the same provider-neutral tool. */
    return runOptions.schema === undefined
      ? undefined
      : createStructuredOutputTool(
          options.provider.metadata.id,
          runOptions.schema,
          definitions,
        );
  };

  const pushToolResult = (
    callId: string,
    content: string,
    toolResultStatus?: 'incomplete',
  ): void => {
    /** Tool results retain the provider call ID for the next model turn. */
    options.messages.push({
      role: 'tool',
      toolCallId: callId,
      content,
      ...(toolResultStatus === undefined ? {} : { toolResultStatus }),
    });
  };

  const storeAssistant = (finish: ProviderFinished<unknown>): void => {
    /** Message storage preserves both semantic calls and opaque provider replay. */
    options.messages.push({
      role: 'assistant',
      content:
        finish.text.length > 0 ? finish.text : (finish.refusal ?? finish.text),
      ...(finish.toolCalls.length === 0 ? {} : { toolCalls: finish.toolCalls }),
      ...(finish.replay === undefined ? {} : { replay: finish.replay }),
    });
  };

  const validatedCalls = (finish: ProviderFinished<unknown>) =>
    options.tools.calls(finish).map((call) => options.tools.validate(call));

  const runTools = async (
    calls: ReturnType<typeof validatedCalls>,
  ): Promise<void> => {
    /** Execute provider-requested tools sequentially in provider order. */
    for (const call of calls) {
      const result = await options.tools.execute(call);
      pushToolResult(call.id, serializeToolResult(result));
    }
  };

  return {
    complete: async <Output = JsonValue>(
      input: string,
      runOptions: AgentRunOptions<Output> = {},
    ): Promise<AgentResponse<Output>> => {
      const beginTurn = turnGuard(runOptions.maxTurns);
      acquire();

      try {
        /** The caller input starts the run-local conversation. */
        options.messages.push({ role: 'user', content: input });

        /** This value remains stable across every provider turn in the run. */
        const terminal = outputTool(runOptions);
        const maxRepairs = repairLimit(runOptions.maxToolCallRepairs);
        let invalidSubmissions = 0;
        let structuredAttempts = 0;
        let correction: string | undefined;

        while (true) {
          /** Ask the provider for either executable calls or the terminal result. */
          beginTurn();
          const request = buildRequest(runOptions, terminal, correction);
          correction = undefined;
          const finish = await options.provider.complete(request);

          /** Intercept and validate the reserved terminal call before normal tools. */
          if (terminal !== undefined) {
            const submission = parseStructuredOutputTool<Output>(
              finish,
              terminal,
            );

            if (submission.type === 'invalid') {
              storeAssistant(finish);
              let repair;
              try {
                repair = nextStructuredOutputRepair(
                  terminal,
                  submission.error,
                  invalidSubmissions,
                  maxRepairs,
                );
              } catch (error) {
                structuredAttempts += 1;
                await notifyStructuredAttempt(runOptions, {
                  attempt: structuredAttempts,
                  runtimeAccepted: false,
                  feedbackSent: false,
                  ...(submission.error.data.diagnostic === undefined
                    ? {}
                    : { diagnostic: submission.error.data.diagnostic }),
                });
                throw error;
              }
              structuredAttempts += 1;
              await notifyStructuredAttempt(runOptions, {
                attempt: structuredAttempts,
                runtimeAccepted: false,
                feedbackSent: true,
                ...(submission.error.data.diagnostic === undefined
                  ? {}
                  : { diagnostic: submission.error.data.diagnostic }),
              });
              invalidSubmissions = repair.invalidSubmissions;
              pushIncompleteToolResults(finish, pushToolResult);
              await notifyToolCallRepair(runOptions, {
                attempt: invalidSubmissions,
                maxAttempts: maxRepairs,
              });
              correction = repair.correction;
              continue;
            }

            if (submission.type === 'finished') {
              structuredAttempts += 1;
              await notifyStructuredAttempt(runOptions, {
                attempt: structuredAttempts,
                runtimeAccepted: true,
                feedbackSent: false,
              });
              /** Store the normalized tool-free finish as the final assistant message. */
              storeAssistant(submission.finish);
              return responseFromFinish(submission.finish);
            }
          }

          /** Ordinary assistant turns remain available to later tool iterations. */
          storeAssistant(finish);

          if (finish.toolCalls.length === 0) {
            return responseFromFinish(finish);
          }

          let calls: ReturnType<typeof validatedCalls>;
          try {
            calls = validatedCalls(finish);
          } catch (error) {
            if (invalidSubmissions >= maxRepairs) throw error;
            invalidSubmissions += 1;
            pushIncompleteToolResults(finish, pushToolResult);
            await notifyToolCallRepair(runOptions, {
              attempt: invalidSubmissions,
              maxAttempts: maxRepairs,
            });
            correction = toolCallCorrection;
            continue;
          }

          /** Tool results are appended before the loop requests the next turn. */
          await runTools(calls);
        }
      } finally {
        release();
      }
    },

    stream: async function* <Output = JsonValue>(
      input: string,
      runOptions: AgentRunOptions<Output> = {},
    ) {
      const beginTurn = turnGuard(runOptions.maxTurns);
      acquire();

      try {
        /** Streaming uses the same message and terminal-tool contract as complete. */
        options.messages.push({ role: 'user', content: input });
        const terminal = outputTool(runOptions);
        const maxRepairs = repairLimit(runOptions.maxToolCallRepairs);
        let invalidSubmissions = 0;
        let structuredAttempts = 0;
        let correction: string | undefined;
        yield {
          type: 'agent.started',
          model: options.model,
          input,
        } as const;

        while (true) {
          let finish: ProviderFinished<Output> | undefined;
          let rejected = false;
          beginTurn();
          const request = buildRequest(runOptions, terminal, correction);
          correction = undefined;

          for await (const event of options.provider.stream(request)) {
            if (rejected) continue;

            if (event.type === 'response.finished') {
              /** Normalize a terminal call before exposing the finished event. */
              if (terminal !== undefined) {
                const submission = parseStructuredOutputTool<Output>(
                  event.finish,
                  terminal,
                );

                if (submission.type === 'invalid') {
                  storeAssistant(event.finish);
                  let repair;
                  try {
                    repair = nextStructuredOutputRepair(
                      terminal,
                      submission.error,
                      invalidSubmissions,
                      maxRepairs,
                    );
                  } catch (error) {
                    structuredAttempts += 1;
                    await notifyStructuredAttempt(runOptions, {
                      attempt: structuredAttempts,
                      runtimeAccepted: false,
                      feedbackSent: false,
                      ...(submission.error.data.diagnostic === undefined
                        ? {}
                        : { diagnostic: submission.error.data.diagnostic }),
                    });
                    throw error;
                  }
                  structuredAttempts += 1;
                  await notifyStructuredAttempt(runOptions, {
                    attempt: structuredAttempts,
                    runtimeAccepted: false,
                    feedbackSent: true,
                    ...(submission.error.data.diagnostic === undefined
                      ? {}
                      : { diagnostic: submission.error.data.diagnostic }),
                  });
                  invalidSubmissions = repair.invalidSubmissions;
                  pushIncompleteToolResults(event.finish, pushToolResult);
                  await notifyToolCallRepair(runOptions, {
                    attempt: invalidSubmissions,
                    maxAttempts: maxRepairs,
                  });
                  correction = repair.correction;
                  rejected = true;
                  continue;
                }

                finish =
                  submission.type === 'finished'
                    ? submission.finish
                    : event.finish;
                if (submission.type === 'finished') {
                  structuredAttempts += 1;
                  await notifyStructuredAttempt(runOptions, {
                    attempt: structuredAttempts,
                    runtimeAccepted: true,
                    feedbackSent: false,
                  });
                }
              } else {
                finish = event.finish;
              }

              yield { ...event, finish };
              continue;
            }

            yield event;
          }

          if (rejected) continue;

          if (finish === undefined) {
            throw new AgentErrorObject({
              code: 'missing_provider_finish',
              message:
                'Provider stream ended without a response.finished event.',
            });
          }

          storeAssistant(finish);

          /** A normalized terminal finish has no calls and completes the stream. */
          let calls: ReturnType<typeof validatedCalls>;
          try {
            calls = validatedCalls(finish);
          } catch (error) {
            if (invalidSubmissions >= maxRepairs) throw error;
            invalidSubmissions += 1;
            pushIncompleteToolResults(finish, pushToolResult);
            await notifyToolCallRepair(runOptions, {
              attempt: invalidSubmissions,
              maxAttempts: maxRepairs,
            });
            correction = toolCallCorrection;
            continue;
          }

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

const defaultMaxToolCallRepairs = 2;

const repairLimit = (value: number | undefined): number => {
  const limit = value ?? defaultMaxToolCallRepairs;

  if (Number.isSafeInteger(limit) && limit >= 0) return limit;
  throw new TypeError(
    'Agent maxToolCallRepairs must be a non-negative safe integer.',
  );
};

const toolCallCorrection = [
  '# Tool call correction',
  '',
  'The previous tool-call response was rejected before any tool ran.',
  'Call only available tools and pass valid JSON arguments matching their schemas.',
].join('\n');

const pushIncompleteToolResults = (
  finish: ProviderFinished<unknown>,
  push: (callId: string, content: string, status: 'incomplete') => void,
): void => {
  for (const call of finish.toolCalls) {
    push(
      call.id,
      'Tool call rejected before execution. Correct the call and try again.',
      'incomplete',
    );
  }
};

const notifyToolCallRepair = async <Output>(
  options: AgentRunOptions<Output>,
  event: Omit<AgentToolCallRepairEvent, 'schemaVersion'>,
): Promise<void> => {
  await options.onToolCallRepair?.({ schemaVersion: 1, ...event });
};

const notifyStructuredAttempt = async <Output>(
  options: AgentRunOptions<Output>,
  event: Omit<AgentStructuredAttemptEvent, 'schemaVersion'>,
): Promise<void> => {
  await options.onStructuredAttempt?.({ schemaVersion: 1, ...event });
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
