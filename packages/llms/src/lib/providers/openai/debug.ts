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
): Promise<ProviderFinished<Output>> => {
  await logger?.log({
    provider: debugProvider,
    target: 'responses',
    event: 'response.finish',
    fields: { source, finish: finishDebugFields(finish) },
  });

  try {
    return parseStructuredOutput('openai', request, finish);
  } catch (error) {
    await logger?.log({
      provider: debugProvider,
      target: 'responses',
      event: 'structured_output.error',
      fields: {
        source,
        finish: finishDebugFields(finish),
        error: errorDebugFields(error),
      },
    });
    throw error;
  }
};

const finishDebugFields = (
  finish: ProviderFinished,
): Record<string, unknown> => ({
  finishReason: finish.finishReason,
  textLength: finish.text.length,
  textExcerpt: diagnosticExcerpt(finish.text, 512),
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

const errorDebugFields = (error: unknown): Record<string, unknown> => {
  if (error instanceof ProviderErrorObject) {
    return {
      name: error.name,
      code: error.data.code,
      message: error.data.message,
      diagnostic: error.data.diagnostic,
      cause: causeDebugFields(error),
    };
  }

  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      cause: causeDebugFields(error),
    };
  }

  return { value: String(error) };
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
