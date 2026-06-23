import { ProviderErrorObject } from '../classes/provider-error.js';
import type { LlmDebugLogger } from '../debug.js';
import type { HttpTransport } from '../types/http.js';
import type {
  JsonObject,
  JsonValue,
  LlmProvider,
  Model,
  ProviderCapabilities,
  ProviderFinished,
  ProviderMessage,
  ProviderMetadata,
  ProviderRequest,
  ProviderId,
  ProviderStreamEvent,
  ProviderToolCall,
  ReasoningMetadata,
} from '../types/provider.js';
import {
  asRecord,
  arrayField,
  recordField,
  stringField,
} from '../utils/json.js';
import { diagnosticExcerpt } from '../utils/diagnostics.js';
import { parseSseEvents } from '../utils/sse.js';
import {
  finishReason,
  httpError,
  messageText,
  parseJsonBody,
  parseStructuredOutput,
  parseUsage,
  requireRequestInput,
  streamErrorEvent,
  structuredJsonSchema,
} from './common.js';

const structuredOutputName = 'structured_output';

export type SecretSource = string | (() => string | Promise<string>);

export type OpenAiProviderDeps = {
  readonly transport: HttpTransport;
  readonly apiKey?: SecretSource;
  readonly authorization?: SecretSource;
  readonly baseUrl?: string;
  readonly debugLogger?: LlmDebugLogger;
  readonly debugProviderId?: ProviderId;
};

export const openAiMetadata: ProviderMetadata = {
  id: 'openai',
  name: 'OpenAI',
  baseUrl: 'https://api.openai.com/v1',
};

export const openAiCapabilities: ProviderCapabilities = {
  streaming: true,
  tools: true,
  reasoning: true,
  modelListing: true,
  oauth: false,
  serviceTier: true,
  structuredOutputs: true,
};

export const createOpenAiProvider = (deps: OpenAiProviderDeps): LlmProvider => {
  const baseUrl = deps.baseUrl ?? openAiMetadata.baseUrl;
  const logger = deps.debugLogger;
  const debugProvider = deps.debugProviderId ?? 'openai';

  const send = async (
    request: ProviderRequest<unknown>,
    body: Record<string, unknown>,
  ): Promise<Record<string, unknown>> => {
    const response = await deps.transport.request({
      method: 'POST',
      url: `${baseUrl}/responses`,
      headers: {
        authorization: await authorization(deps),
        'content-type': 'application/json',
        accept: 'application/json',
      },
      body: JSON.stringify(body),
      signal: request.signal,
    });

    await logger?.log({
      provider: debugProvider,
      target: 'responses',
      event: 'http.response',
      fields: { status: response.status, body: response.body },
    });

    if (response.status >= 400) {
      throw httpError('openai', response.status, response.body);
    }

    return parseJsonBody('openai', response.body);
  };

  return {
    metadata: openAiMetadata,
    capabilities: openAiCapabilities,

    async complete<Output = JsonValue>(
      request: ProviderRequest<Output>,
    ): Promise<ProviderFinished<Output>> {
      requireRequestInput('openai', request);
      const body = openAiBody(request, false);
      await logger?.log({
        provider: debugProvider,
        target: 'responses',
        event: 'http.request',
        fields: { body },
      });

      return parseOpenAiStructuredOutput(
        logger,
        debugProvider,
        request,
        parseOpenAiFinished(await send(request, body)),
        'complete',
      );
    },

    async *stream<Output = JsonValue>(
      request: ProviderRequest<Output>,
    ): AsyncIterable<ProviderStreamEvent<Output>> {
      requireRequestInput('openai', request);
      const body = openAiBody(request, true);
      const text: string[] = [];
      const reasoning: string[] = [];
      const refusals: string[] = [];
      const textSnapshots: OpenAiTextSnapshots = {
        outputItems: [],
        outputTexts: [],
        contentParts: [],
      };
      const calls = new Map<number, ProviderToolCall>();
      await logger?.log({
        provider: debugProvider,
        target: 'responses',
        event: 'http.request',
        fields: { body },
      });
      const auth = await authorization(deps);

      yield {
        type: 'response.started',
        provider: 'openai',
        model: body.model as string,
      };

      for await (const event of parseSseEvents(
        deps.transport.stream({
          method: 'POST',
          url: `${baseUrl}/responses`,
          headers: {
            authorization: auth,
            'content-type': 'application/json',
            accept: 'text/event-stream',
          },
          body: JSON.stringify(body),
          signal: request.signal,
        }),
      )) {
        if (event.done) {
          break;
        }

        let payload: Record<string, unknown>;

        try {
          payload = parseJsonBody('openai', event.data);
        } catch {
          await logger?.log({
            provider: debugProvider,
            target: 'responses',
            event: 'stream.event.invalid_json',
            fields: { data: event.data },
          });
          yield streamErrorEvent(
            'openai',
            'malformed_stream_event',
            event.data,
          );
          return;
        }

        const parsed = openAiStreamEvent(payload, calls);
        recordOpenAiTextSnapshot(textSnapshots, payload);

        if (parsed?.type === 'text.delta') {
          text.push(parsed.delta);
        }

        if (parsed?.type === 'reasoning.delta') {
          reasoning.push(parsed.delta);
        }

        if (parsed?.type === 'refusal.delta') {
          refusals.push(parsed.delta);
        }

        if (parsed !== undefined) {
          yield parsed;
        }

        if (payload.type === 'response.completed') {
          await logger?.log({
            provider: debugProvider,
            target: 'responses',
            event: 'stream.response.completed',
            fields: { payload },
          });
          const response = recordField(payload, 'response') ?? payload;
          const finish = await parseOpenAiStructuredOutput(
            logger,
            debugProvider,
            request,
            parseOpenAiFinished(
              response,
              openAiStreamText(text, textSnapshots),
              reasoning.join(''),
              refusals.join('') || undefined,
              [...calls.values()],
            ),
            'stream',
          );
          const usage = finish.usage;

          if (usage !== undefined) {
            yield { type: 'usage', usage };
          }

          yield { type: 'response.finished', finish };
          return;
        }

        if (payload.type === 'response.failed') {
          await logger?.log({
            provider: debugProvider,
            target: 'responses',
            event: 'stream.response.failed',
            fields: { payload },
          });
          yield streamErrorEvent(
            'openai',
            'provider_error',
            'OpenAI stream failed.',
          );
          return;
        }
      }

      yield {
        type: 'response.finished',
        finish: await parseOpenAiStructuredOutput(
          logger,
          debugProvider,
          request,
          {
            text: openAiStreamText(text, textSnapshots) ?? '',
            reasoning: { text: reasoning.join('') },
            refusal: refusals.join('') || undefined,
            finishReason: 'unknown',
            toolCalls: [...calls.values()],
          },
          'stream_end',
        ),
      };
    },

    async models(signal?: AbortSignal): Promise<readonly Model[]> {
      const response = await deps.transport.request({
        method: 'GET',
        url: `${baseUrl}/models`,
        headers: {
          authorization: await authorization(deps),
          accept: 'application/json',
        },
        signal,
      });

      if (response.status >= 400) {
        throw httpError('openai', response.status, response.body);
      }

      return arrayField(parseJsonBody('openai', response.body), 'data')
        .map(asRecord)
        .filter(
          (model): model is Record<string, unknown> => model !== undefined,
        )
        .map((model) => ({
          id: stringField(model, 'id') ?? '',
          name: stringField(model, 'id'),
          provider: 'openai',
          raw: model,
        }))
        .filter((model) => model.id !== '');
    },

    async validateModel(model: string, signal?: AbortSignal): Promise<Model> {
      const found = (await this.models(signal)).find(
        (item) => item.id === model,
      );

      if (found === undefined) {
        throw new ProviderErrorObject({
          provider: 'openai',
          code: 'missing_model',
          message: `OpenAI model is not available: ${model}`,
        });
      }

      return found;
    },
  };
};

export const openAiBody = (
  request: ProviderRequest<unknown>,
  stream: boolean,
): Record<string, unknown> => {
  requireRequestInput('openai', request);
  const schema = structuredJsonSchema('openai', request.schema);
  const strictSchema =
    schema === undefined
      ? undefined
      : (openAiStrictSchema(schema) as JsonObject);
  const alias = fastAlias(request.model);
  const system = request.messages
    .filter((message) => message.role === 'system')
    .map(messageText)
    .filter((text) => text !== '')
    .join('\n\n');
  const nonSystem = request.messages.filter(
    (message) => message.role !== 'system',
  );
  const reasoning = reasoningRequest(request);

  return prune({
    model: alias.model,
    instructions: system === '' ? undefined : system,
    input: nonSystem.flatMap(openAiInputItems),
    tools: request.tools?.map((tool) => ({
      type: 'function',
      name: tool.name,
      description: tool.description,
      parameters: openAiToolParameters(tool.inputSchema, tool.strict),
      strict: tool.strict,
    })),
    text:
      request.schema === undefined
        ? undefined
        : {
            format: prune({
              type: 'json_schema',
              name: structuredOutputName,
              strict: true,
              schema: strictSchema,
            }),
          },
    temperature: request.temperature,
    max_output_tokens: request.maxOutputTokens,
    service_tier: request.flags?.serviceTier ?? alias.serviceTier,
    reasoning,
    stream,
  });
};

const openAiInputItems = (
  message: ProviderMessage,
): readonly Record<string, unknown>[] => {
  if (message.role === 'tool') {
    return [
      {
        type: 'function_call_output',
        call_id: message.toolCallId,
        output: messageText(message),
      },
    ];
  }

  const text = messageText(message);
  const item = {
    role: message.role,
    content: [
      {
        type: message.role === 'assistant' ? 'output_text' : 'input_text',
        text,
      },
    ],
  };

  if (message.role !== 'assistant') {
    return [item];
  }

  const calls = message.toolCalls?.map(openAiFunctionCallItem) ?? [];

  if (calls.length === 0) {
    return [item];
  }

  return text === '' ? calls : [item, ...calls];
};

const openAiFunctionCallItem = (
  call: ProviderToolCall,
): Record<string, unknown> => ({
  type: 'function_call',
  call_id: call.id,
  name: call.name,
  arguments: call.arguments,
});

const parseOpenAiStructuredOutput = async <Output = JsonValue>(
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
    fields: { source, finish: openAiFinishDebugFields(finish) },
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
        finish: openAiFinishDebugFields(finish),
        error: openAiErrorDebugFields(error),
      },
    });
    throw error;
  }
};

const openAiFinishDebugFields = (
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

const openAiErrorDebugFields = (error: unknown): Record<string, unknown> => {
  if (error instanceof ProviderErrorObject) {
    return {
      name: error.name,
      code: error.data.code,
      message: error.data.message,
      diagnostic: error.data.diagnostic,
      cause: openAiCauseDebugFields(error),
    };
  }

  if (error instanceof Error) {
    return {
      name: error.name,
      message: error.message,
      cause: openAiCauseDebugFields(error),
    };
  }

  return { value: String(error) };
};

const openAiCauseDebugFields = (
  error: Error & { readonly cause?: unknown },
): Record<string, unknown> | undefined => {
  const cause = error.cause;

  if (cause instanceof Error) {
    return { name: cause.name, message: cause.message };
  }

  return cause === undefined ? undefined : { value: String(cause) };
};

const openAiStreamEvent = (
  payload: Record<string, unknown>,
  calls: Map<number, ProviderToolCall>,
):
  | Exclude<ProviderStreamEvent, { readonly type: 'response.finished' }>
  | undefined => {
  const type = stringField(payload, 'type');

  if (type === 'response.output_text.delta') {
    return { type: 'text.delta', delta: stringField(payload, 'delta') ?? '' };
  }

  if (
    type === 'response.reasoning_summary_text.delta' ||
    type === 'response.reasoning_text.delta'
  ) {
    return {
      type: 'reasoning.delta',
      delta: stringField(payload, 'delta') ?? '',
    };
  }

  if (type === 'response.refusal.delta') {
    return {
      type: 'refusal.delta',
      delta: stringField(payload, 'delta') ?? '',
    };
  }

  if (type === 'response.function_call_arguments.delta') {
    const index = Number(payload.output_index ?? payload.item_index ?? 0);
    const previous = calls.get(index);
    const delta = stringField(payload, 'delta') ?? '';
    const next = {
      id: previous?.id ?? stringField(payload, 'item_id') ?? `call_${index}`,
      name: previous?.name ?? stringField(payload, 'name') ?? '',
      arguments: `${previous?.arguments ?? ''}${delta}`,
      index,
    };

    calls.set(index, next);

    return {
      type: 'tool_call.delta',
      index,
      id: next.id,
      name: next.name,
      argumentsDelta: delta,
    };
  }

  if (type === 'response.output_item.done') {
    const item = recordField(payload, 'item');

    if (item?.type !== 'function_call') {
      return undefined;
    }

    const index = Number(payload.output_index ?? 0);
    const call = {
      id:
        stringField(item, 'call_id') ??
        stringField(item, 'id') ??
        `call_${index}`,
      name: stringField(item, 'name') ?? '',
      arguments:
        stringField(item, 'arguments') ?? calls.get(index)?.arguments ?? '',
      index,
    };

    calls.set(index, call);

    return { type: 'tool_call.done', call };
  }

  return undefined;
};

type OpenAiTextSnapshots = {
  readonly outputItems: string[];
  readonly outputTexts: string[];
  readonly contentParts: string[];
};

const recordOpenAiTextSnapshot = (
  snapshots: OpenAiTextSnapshots,
  payload: Record<string, unknown>,
): void => {
  const type = stringField(payload, 'type');
  const text = openAiTextSnapshot(type, payload);

  if (text === undefined || text === '') {
    return;
  }

  if (type === 'response.output_item.done') {
    snapshots.outputItems.push(text);
    return;
  }

  if (type === 'response.output_text.done') {
    snapshots.outputTexts.push(text);
    return;
  }

  snapshots.contentParts.push(text);
};

const openAiTextSnapshot = (
  type: string | undefined,
  payload: Record<string, unknown>,
): string | undefined => {
  if (type === 'response.output_text.done') {
    return stringField(payload, 'text');
  }

  if (type === 'response.content_part.done') {
    return openAiContentPartText(recordField(payload, 'part'));
  }

  if (type === 'response.output_item.done') {
    return openAiOutputItemText(recordField(payload, 'item'));
  }

  return undefined;
};

const openAiStreamText = (
  deltas: readonly string[],
  snapshots: OpenAiTextSnapshots,
): string | undefined => {
  const deltaText = deltas.join('');

  if (deltaText !== '') {
    return deltaText;
  }

  const snapshotText =
    snapshotGroupText(snapshots.outputItems) ??
    snapshotGroupText(snapshots.outputTexts) ??
    snapshotGroupText(snapshots.contentParts);

  return snapshotText;
};

const snapshotGroupText = (items: readonly string[]): string | undefined => {
  const text = items.join('');

  return text === '' ? undefined : text;
};

const parseOpenAiFinished = (
  response: Record<string, unknown>,
  streamText?: string,
  streamReasoning?: string,
  streamRefusal?: string,
  streamToolCalls: readonly ProviderToolCall[] = [],
): ProviderFinished => {
  const output = arrayField(response, 'output').map(asRecord).filter(isRecord);
  const content = output
    .flatMap((item) => arrayField(item, 'content'))
    .map(asRecord)
    .filter(isRecord);
  const contentText = content
    .filter((item) => item.type === 'output_text')
    .map((item) => stringField(item, 'text') ?? '')
    .join('');
  const outputText =
    streamText ??
    nonEmptyText(stringField(response, 'output_text')) ??
    contentText;
  const refusal =
    stringField(response, 'refusal') ??
    streamRefusal ??
    content
      .filter((item) => item.type === 'refusal')
      .map(
        (item) =>
          stringField(item, 'refusal') ?? stringField(item, 'text') ?? '',
      )
      .join('');
  const responseToolCalls = output
    .filter((item) => item.type === 'function_call')
    .map((item, index) => ({
      id:
        stringField(item, 'call_id') ??
        stringField(item, 'id') ??
        `call_${index}`,
      name: stringField(item, 'name') ?? '',
      arguments: stringField(item, 'arguments') ?? '',
      index,
    }));
  const toolCalls =
    responseToolCalls.length === 0 ? streamToolCalls : responseToolCalls;
  const reasoningText =
    streamReasoning ??
    output
      .filter((item) => item.type === 'reasoning')
      .flatMap((item) => arrayField(item, 'summary'))
      .map(asRecord)
      .filter(isRecord)
      .map((summary) => stringField(summary, 'text') ?? '')
      .join('');
  const usage = parseUsage(recordField(response, 'usage'));
  const reasoning: ReasoningMetadata | undefined =
    reasoningText === '' ? undefined : { text: reasoningText };

  return {
    text: outputText,
    finishReason: finishReason(
      response.status === 'completed' ? 'stop' : response.status,
    ),
    usage,
    reasoning,
    refusal: refusal === '' ? undefined : refusal,
    toolCalls,
  };
};

const openAiOutputItemText = (
  item: Record<string, unknown> | undefined,
): string | undefined => {
  if (item?.type === 'output_text') {
    return stringField(item, 'text');
  }

  if (item?.type !== 'message') {
    return undefined;
  }

  return openAiContentText(item);
};

const openAiContentPartText = (
  part: Record<string, unknown> | undefined,
): string | undefined =>
  part?.type === 'output_text' ? stringField(part, 'text') : undefined;

const openAiContentText = (item: Record<string, unknown>): string => {
  return arrayField(item, 'content')
    .map(asRecord)
    .filter(isRecord)
    .filter((content) => content.type === 'output_text')
    .map((content) => stringField(content, 'text') ?? '')
    .join('');
};

const nonEmptyText = (value: string | undefined): string | undefined =>
  value === '' ? undefined : value;

const authorization = async (deps: OpenAiProviderDeps): Promise<string> => {
  if (deps.apiKey !== undefined && deps.authorization !== undefined) {
    throw new ProviderErrorObject({
      provider: 'openai',
      code: 'auth_ambiguous',
      message:
        'OpenAI provider accepts either apiKey or authorization, not both.',
    });
  }

  if (deps.apiKey !== undefined) {
    return `Bearer ${await secret(deps.apiKey)}`;
  }

  if (deps.authorization !== undefined) {
    return secret(deps.authorization);
  }

  throw new ProviderErrorObject({
    provider: 'openai',
    code: 'auth_missing',
    message: 'OpenAI provider requires an API key or authorization header.',
  });
};

const secret = async (source: SecretSource): Promise<string> => {
  const value = typeof source === 'function' ? await source() : source;

  if (value.trim() === '') {
    throw new ProviderErrorObject({
      provider: 'openai',
      code: 'auth_missing',
      message: 'OpenAI provider received an empty auth value.',
    });
  }

  return value;
};

const fastAlias = (
  model: string,
): { readonly model: string; readonly serviceTier?: 'priority' } =>
  model.endsWith('-fast')
    ? { model: model.slice(0, -5), serviceTier: 'priority' }
    : { model };

const reasoningRequest = (
  request: ProviderRequest<unknown>,
): JsonObject | undefined => {
  const value = request.flags?.reasoning;

  if (value === undefined || value === false) {
    return undefined;
  }

  if (value === true) {
    return {};
  }

  return prune({
    effort: value.effort,
    summary: value.summary,
  }) as JsonObject;
};

const openAiToolParameters = (
  schema: JsonObject,
  strict: boolean | undefined,
): JsonObject =>
  strict === true ? (openAiStrictSchema(schema) as JsonObject) : schema;

const openAiStrictSchema = (value: JsonValue): JsonValue => {
  if (Array.isArray(value)) {
    return value.map(openAiStrictSchema);
  }

  const record = asRecord(value);

  if (record === undefined) {
    return value;
  }

  const schema = Object.fromEntries(
    Object.entries(record).map(([key, child]) => [
      key,
      openAiStrictSchema(child as JsonValue),
    ]),
  ) as JsonObject;
  const properties = asRecord(schema.properties);

  if (schema.type !== 'object' || properties === undefined) {
    return schema;
  }

  return {
    ...schema,
    required: Object.keys(properties),
    additionalProperties: false,
  };
};

const prune = (value: Record<string, unknown>): Record<string, unknown> =>
  Object.fromEntries(
    Object.entries(value).filter(([, child]) => child !== undefined),
  );

const isRecord = (
  value: Record<string, unknown> | undefined,
): value is Record<string, unknown> => value !== undefined;
