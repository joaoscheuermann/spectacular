import { ProviderErrorObject } from '../classes/provider-error.js';
import type {
  FinishReason,
  JsonObject,
  JsonValue,
  ProviderError,
  ProviderFinished,
  ProviderId,
  ProviderMessage,
  ProviderRequest,
  ReasoningEffort,
  StructuredOutputSchema,
  UsageMetadata,
} from '../types/provider.js';
import { diagnosticExcerpt } from '../utils/diagnostics.js';
import { asRecord, numberField, recordField } from '../utils/json.js';
import { z } from 'zod';

export const requireRequestInput = (
  provider: ProviderId,
  request: ProviderRequest<unknown>,
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

export const requestReasoningEffort = (
  request: ProviderRequest<unknown>,
): ReasoningEffort | undefined => {
  if (request.effort !== undefined) {
    return request.effort;
  }

  const value = request.flags?.reasoning;

  return typeof value === 'object' ? value.effort : undefined;
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
    diagnostic:
      diagnostic === undefined ? undefined : diagnosticExcerpt(diagnostic),
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
    (details === undefined
      ? undefined
      : numberField(details, 'reasoning_tokens'));
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

export const structuredJsonSchema = (
  provider: ProviderId,
  schema: StructuredOutputSchema | undefined,
): JsonObject | undefined => {
  if (schema === undefined) {
    return undefined;
  }

  if (!(schema instanceof z.ZodType)) {
    throw new ProviderErrorObject({
      provider,
      code: 'invalid_structured_schema',
      message: `${provider} structured output schema must be a Zod schema.`,
    });
  }

  let value: unknown;

  try {
    value = z.toJSONSchema(schema, {
      io: 'output',
      unrepresentable: 'throw',
    });
  } catch (cause) {
    throw new ProviderErrorObject(
      {
        provider,
        code: 'invalid_structured_schema',
        message: `${provider} structured output schema cannot be represented as JSON Schema.`,
      },
      { cause },
    );
  }

  const json = asJsonObject(value);

  if (json?.type !== 'object') {
    throw new ProviderErrorObject({
      provider,
      code: 'invalid_structured_schema',
      message: `${provider} structured output schema must produce an object JSON Schema.`,
    });
  }

  return json;
};

export const parseStructuredOutput = <Output = JsonValue>(
  provider: ProviderId,
  request: ProviderRequest<Output>,
  finish: ProviderFinished,
): ProviderFinished<Output> => {
  const schema = request.schema;

  if (schema === undefined) {
    return finish as ProviderFinished<Output>;
  }

  if (finish.refusal !== undefined || finish.toolCalls.length > 0) {
    return finish as ProviderFinished<Output>;
  }

  let parsed: unknown;

  try {
    parsed = JSON.parse(finish.text) as unknown;
  } catch (cause) {
    throw new ProviderErrorObject(
      {
        provider,
        code: 'invalid_structured_output',
        message: `${provider} returned invalid structured output.`,
        diagnostic: diagnosticExcerpt(finish.text),
      },
      { cause },
    );
  }

  return {
    ...finish,
    structured: validateStructuredOutput(provider, schema, parsed) as Output,
  };
};

const validateStructuredOutput = <Schema extends StructuredOutputSchema>(
  provider: ProviderId,
  schema: Schema,
  value: unknown,
): z.output<Schema> => {
  const parsed = schema.safeParse(value);

  if (parsed.success) {
    return parsed.data;
  }

  throw new ProviderErrorObject({
    provider,
    code: 'invalid_structured_output',
    message: `${provider} structured output failed schema validation.`,
    diagnostic: parsed.error.issues.map(issueDiagnostic).join('; '),
  });
};

const issueDiagnostic = (issue: z.core.$ZodIssue): string => {
  const path = issue.path.map(String).join('.');

  return path === '' ? issue.message : `${path}: ${issue.message}`;
};

const isJsonValue = (value: unknown): value is JsonValue => {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'number' ||
    typeof value === 'boolean'
  ) {
    return Number.isFinite(value) || typeof value !== 'number';
  }

  if (Array.isArray(value)) {
    return value.every(isJsonValue);
  }

  return (
    value !== null &&
    typeof value === 'object' &&
    !Array.isArray(value) &&
    Object.values(value).every(isJsonValue)
  );
};

const asJsonObject = (value: unknown): JsonObject | undefined =>
  value !== null &&
  typeof value === 'object' &&
  !Array.isArray(value) &&
  isJsonValue(value)
    ? (value as JsonObject)
    : undefined;
