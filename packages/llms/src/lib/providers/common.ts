import { ProviderErrorObject } from '../classes/provider-error.js';
import type {
  FinishReason,
  ProviderError,
  ProviderId,
  ProviderMessage,
  ProviderRequest,
  UsageMetadata,
} from '../types/provider.js';
import { diagnosticExcerpt } from '../utils/diagnostics.js';
import { asRecord, numberField, recordField } from '../utils/json.js';

export const requireRequestInput = (
  provider: ProviderId,
  request: ProviderRequest,
): void => {
  if (request.model.trim() === '') {
    throw new ProviderErrorObject({
      provider,
      code: 'missing_model',
      message: 'Provider request requires a model.',
    });
  }

  if (request.messages.length === 0) {
    throw new ProviderErrorObject({
      provider,
      code: 'missing_input',
      message: 'Provider request requires at least one message.',
    });
  }
};

export const messageText = (message: ProviderMessage): string => {
  if (typeof message.content === 'string') {
    return message.content;
  }

  return (message.content ?? [])
    .filter((part) => part.type === 'text')
    .map((part) => part.text)
    .join('\n');
};

export const httpError = (
  provider: ProviderId,
  status: number,
  body: string,
): ProviderErrorObject =>
  new ProviderErrorObject({
    provider,
    code: status === 401 || status === 403 ? 'auth_failed' : 'http_error',
    message: `${provider} request failed with HTTP ${status}.`,
    status,
    retryable: status === 429 || status >= 500,
    diagnostic: diagnosticExcerpt(body),
  });

export const streamErrorEvent = (
  provider: ProviderId,
  code: string,
  message: string,
  diagnostic?: string,
): { readonly type: 'error'; readonly error: ProviderError } => ({
  type: 'error',
  error: {
    provider,
    code,
    message,
    diagnostic: diagnostic === undefined ? undefined : diagnosticExcerpt(diagnostic),
  },
});

export const parseUsage = (
  usage: Record<string, unknown> | undefined,
): UsageMetadata | undefined => {
  if (usage === undefined) {
    return undefined;
  }

  const details =
    recordField(usage, 'completion_tokens_details') ??
    recordField(usage, 'output_tokens_details');
  const promptDetails = recordField(usage, 'prompt_tokens_details');
  const inputTokens =
    numberField(usage, 'input_tokens') ?? numberField(usage, 'prompt_tokens');
  const outputTokens =
    numberField(usage, 'output_tokens') ??
    numberField(usage, 'completion_tokens');
  const totalTokens = numberField(usage, 'total_tokens');
  const reasoningTokens =
    numberField(usage, 'reasoning_tokens') ??
    (details === undefined ? undefined : numberField(details, 'reasoning_tokens'));
  const cachedInputTokens =
    promptDetails === undefined
      ? undefined
      : numberField(promptDetails, 'cached_tokens');

  return {
    inputTokens,
    outputTokens,
    totalTokens,
    reasoningTokens,
    cachedInputTokens,
  };
};

export const finishReason = (value: unknown): FinishReason => {
  if (
    value === 'stop' ||
    value === 'length' ||
    value === 'tool_calls' ||
    value === 'content_filter'
  ) {
    return value;
  }

  if (value === 'cancelled') {
    return 'cancelled';
  }

  if (value === 'error' || value === 'failed') {
    return 'error';
  }

  return 'unknown';
};

export const parseJsonBody = (
  provider: ProviderId,
  body: string,
): Record<string, unknown> => {
  try {
    const parsed = asRecord(JSON.parse(body));

    if (parsed !== undefined) {
      return parsed;
    }
  } catch (cause) {
    throw new ProviderErrorObject(
      {
        provider,
        code: 'invalid_json',
        message: `${provider} returned invalid JSON.`,
        diagnostic: diagnosticExcerpt(body),
      },
      { cause },
    );
  }

  throw new ProviderErrorObject({
    provider,
    code: 'invalid_json',
    message: `${provider} returned a non-object JSON body.`,
    diagnostic: diagnosticExcerpt(body),
  });
};
