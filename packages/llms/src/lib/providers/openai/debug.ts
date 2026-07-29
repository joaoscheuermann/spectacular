import { ProviderErrorObject } from '../../classes/provider-error.js';
import type { LlmDebugLogger } from '../../debug.js';
import type {
  JsonValue,
  ProviderFinished,
  ProviderId,
  ProviderRequest,
} from '../../types/provider.js';
import { diagnosticExcerpt } from '../../utils/diagnostics.js';
import { parseStructuredOutput } from '../common.js';

export const parseStructuredOutputWithDebug = async <Output = JsonValue>(
  logger: LlmDebugLogger | undefined,
  debugProvider: ProviderId,
  request: ProviderRequest<Output>,
  finish: ProviderFinished,
  source: string,
  rejectNonStructured = true,
): Promise<ProviderFinished<Output>> => {
  const sensitiveOutput = request.flags?.sensitiveOutput === true;
  await logger?.log({
    provider: debugProvider,
    target: 'responses',
    event: 'response.finish',
    fields: { source, finish: finishDebugFields(finish, sensitiveOutput) },
  });

  try {
    return parseStructuredOutput(
      'openai',
      request,
      finish,
      rejectNonStructured,
    );
  } catch (error) {
    await logger?.log({
      provider: debugProvider,
      target: 'responses',
      event: 'structured_output.error',
      fields: {
        source,
        finish: finishDebugFields(finish, sensitiveOutput),
        error: errorDebugFields(error, sensitiveOutput),
      },
    });
    throw error;
  }
};

const finishDebugFields = (
  finish: ProviderFinished,
  sensitiveOutput: boolean,
): Record<string, unknown> => ({
  finishReason: finish.finishReason,
  textLength: finish.text.length,
  ...(sensitiveOutput
    ? {}
    : { textExcerpt: diagnosticExcerpt(finish.text, 512) }),
  refusalPresent: finish.refusal !== undefined,
  reasoningPresent: finish.reasoning?.text !== undefined,
  toolCallCount: finish.toolCalls.length,
  toolCalls: finish.toolCalls.map((call) => ({
    id: call.id,
    name: call.name,
    argumentsLength: call.arguments.length,
  })),
  usage: finish.usage,
});

const errorDebugFields = (
  error: unknown,
  sensitiveOutput: boolean,
): Record<string, unknown> => {
  if (error instanceof ProviderErrorObject) {
    return {
      name: error.name,
      code: error.data.code,
      message: error.data.message,
      ...(sensitiveOutput
        ? {}
        : {
            diagnostic: error.data.diagnostic,
            cause: causeDebugFields(error),
          }),
    };
  }

  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      ...(sensitiveOutput ? {} : { cause: causeDebugFields(error) }),
    };
  }

  return sensitiveOutput
    ? { value: 'Sensitive provider error.' }
    : { value: String(error) };
};

const causeDebugFields = (
  error: Error & { readonly cause?: unknown },
): Record<string, unknown> | undefined => {
  const cause = error.cause;

  if (cause instanceof Error) {
    return { name: cause.name, message: cause.message };
  }

  return cause === undefined ? undefined : { value: String(cause) };
};
