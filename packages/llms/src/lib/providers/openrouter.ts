import { ProviderErrorObject } from '../classes/provider-error.js';
import type { LlmDebugLogger } from '../debug.js';
import type { HttpTransport } from '../types/http.js';
import type {
  JsonValue,
  LlmProvider,
  Model,
  ProviderCapabilities,
  ProviderFinished,
  ProviderMetadata,
  ProviderRequest,
  ProviderStreamEvent,
  ProviderToolCall,
} from '../types/provider.js';
import {
  asRecord,
  arrayField,
  numberField,
  recordField,
  stringField,
} from '../utils/json.js';
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

export type OpenRouterProviderDeps = {
  readonly transport: HttpTransport;
  readonly apiKey: string | (() => string | Promise<string>);
  readonly baseUrl?: string;
  readonly debugLogger?: LlmDebugLogger;
};

export const openRouterMetadata: ProviderMetadata = {
  id: 'openrouter',
  name: 'OpenRouter',
  baseUrl: 'https://openrouter.ai/api/v1',
};

export const openRouterCapabilities: ProviderCapabilities = {
  streaming: true,
  tools: true,
  reasoning: true,
  modelListing: true,
  oauth: false,
  serviceTier: false,
  structuredOutputs: true,
};

export const createOpenRouterProvider = (
  deps: OpenRouterProviderDeps,
): LlmProvider => {
  const baseUrl = deps.baseUrl ?? openRouterMetadata.baseUrl;
  const logger = deps.debugLogger;

  const post = async (
    request: ProviderRequest<unknown>,
    body: Record<string, unknown>,
  ): Promise<Record<string, unknown>> => {
    const response = await deps.transport.request({
      method: 'POST',
      url: `${baseUrl}/chat/completions`,
      headers: {
        authorization: `Bearer ${await secret(deps.apiKey)}`,
        'content-type': 'application/json',
        accept: 'application/json',
      },
      body: JSON.stringify(body),
      signal: request.signal,
    });

    await logger?.log({
      provider: 'openrouter',
      target: 'chat/completions',
      event: 'http.response',
      fields: { status: response.status, body: response.body },
    });

    if (response.status >= 400) {
      throw httpError('openrouter', response.status, response.body);
    }

    return parseJsonBody('openrouter', response.body);
  };

  return {
    metadata: openRouterMetadata,
    capabilities: openRouterCapabilities,

    async complete<Output = JsonValue>(
      request: ProviderRequest<Output>,
    ): Promise<ProviderFinished<Output>> {
      requireRequestInput('openrouter', request);
      const body = openRouterBody(request, false);
      await logger?.log({
        provider: 'openrouter',
        target: 'chat/completions',
        event: 'http.request',
        fields: { body },
      });

      return parseStructuredOutput(
        'openrouter',
        request,
        parseOpenRouterFinished(await post(request, body)),
      );
    },

    async *stream<Output = JsonValue>(
      request: ProviderRequest<Output>,
    ): AsyncIterable<ProviderStreamEvent<Output>> {
      requireRequestInput('openrouter', request);
      const body = openRouterBody(request, true);
      const chunks = deps.transport.stream({
        method: 'POST',
        url: `${baseUrl}/chat/completions`,
        headers: {
          authorization: `Bearer ${await secret(deps.apiKey)}`,
          'content-type': 'application/json',
          accept: 'text/event-stream',
        },
        body: JSON.stringify(body),
        signal: request.signal,
      });
      const state: StreamState = {
        text: [],
        reasoning: [],
        refusal: [],
        calls: new Map(),
      };

      yield {
        type: 'response.started',
        provider: 'openrouter',
        model: request.model,
      };

      for await (const event of parseSseEvents(chunks)) {
        if (event.done) {
          break;
        }

        let payload: Record<string, unknown>;

        try {
          payload = parseJsonBody('openrouter', event.data);
        } catch {
          yield streamErrorEvent(
            'openrouter',
            'malformed_stream_event',
            event.data,
          );
          return;
        }

        if (recordField(payload, 'error') !== undefined) {
          yield streamErrorEvent(
            'openrouter',
            'provider_error',
            'OpenRouter stream error.',
            event.data,
          );
          return;
        }

        for (const parsed of openRouterStreamEvents(payload, state)) {
          yield parsed;
        }
      }

      for (const call of [...state.calls.values()]) {
        yield { type: 'tool_call.done', call };
      }

      yield {
        type: 'response.finished',
        finish: parseStructuredOutput('openrouter', request, {
          text: state.text.join(''),
          reasoning: { text: state.reasoning.join('') },
          refusal: state.refusal.join('') || undefined,
          finishReason: state.finishReason ?? 'unknown',
          usage: state.usage,
          toolCalls: [...state.calls.values()],
        }),
      };
    },

    async models(signal?: AbortSignal): Promise<readonly Model[]> {
      const response = await deps.transport.request({
        method: 'GET',
        url: `${baseUrl}/models`,
        headers: {
          authorization: `Bearer ${await secret(deps.apiKey)}`,
          accept: 'application/json',
        },
        signal,
      });

      if (response.status >= 400) {
        throw httpError('openrouter', response.status, response.body);
      }

      return arrayField(parseJsonBody('openrouter', response.body), 'data')
        .map(asRecord)
        .filter(isRecord)
        .map(modelFromRecord);
    },

    async validateModel(model: string, signal?: AbortSignal): Promise<Model> {
      const found = (await this.models(signal)).find(
        (item) => item.id === model,
      );

      if (found === undefined) {
        throw new ProviderErrorObject({
          provider: 'openrouter',
          code: 'missing_model',
          message: `OpenRouter model is not available: ${model}`,
        });
      }

      return found;
    },
  };
};

export const openRouterBody = (
  request: ProviderRequest<unknown>,
  stream: boolean,
): Record<string, unknown> => {
  requireRequestInput('openrouter', request);
  const schema = structuredJsonSchema('openrouter', request.schema);

  return prune({
    model: request.model,
    messages: request.messages.map((message) =>
      prune({
        role: message.role,
        content: messageText(message),
        tool_call_id: message.toolCallId,
        tool_calls: message.toolCalls?.map((call) => ({
          id: call.id,
          type: 'function',
          function: { name: call.name, arguments: call.arguments },
        })),
        name: message.name,
      }),
    ),
    tools: request.tools?.map((tool) => ({
      type: 'function',
      function: prune({
        name: tool.name,
        description: tool.description,
        parameters: tool.inputSchema,
      }),
    })),
    temperature: request.temperature,
    max_tokens: request.maxOutputTokens,
    reasoning: reasoningRequest(request),
    response_format:
      request.schema === undefined
        ? undefined
        : {
            type: 'json_schema',
            json_schema: {
              name: structuredOutputName,
              strict: true,
              schema,
            },
          },
    stream,
    stream_options:
      request.flags?.includeUsage === false
        ? undefined
        : { include_usage: true },
  });
};

const parseOpenRouterFinished = (
  response: Record<string, unknown>,
): ProviderFinished => {
  const choice = asRecord(arrayField(response, 'choices')[0]) ?? {};
  const message = recordField(choice, 'message') ?? {};
  const content = stringField(message, 'content') ?? '';
  const reasoning =
    stringField(message, 'reasoning') ??
    stringField(message, 'reasoning_content');
  const refusal = stringField(message, 'refusal');
  const toolCalls = arrayField(message, 'tool_calls')
    .map(asRecord)
    .filter(isRecord)
    .map((call, index) => {
      const fn = recordField(call, 'function') ?? {};

      return {
        id: stringField(call, 'id') ?? `call_${index}`,
        name: stringField(fn, 'name') ?? '',
        arguments: stringField(fn, 'arguments') ?? '',
        index,
      };
    });

  return {
    text: content,
    finishReason: finishReason(choice.finish_reason),
    usage: parseUsage(recordField(response, 'usage')),
    reasoning: reasoning === undefined ? undefined : { text: reasoning },
    refusal,
    toolCalls,
  };
};

type StreamState = {
  readonly text: string[];
  readonly reasoning: string[];
  readonly refusal: string[];
  readonly calls: Map<number, ProviderToolCall>;
  usage?: ProviderFinished['usage'];
  finishReason?: ProviderFinished['finishReason'];
};

const openRouterStreamEvents = (
  payload: Record<string, unknown>,
  state: StreamState,
): readonly Exclude<
  ProviderStreamEvent,
  { readonly type: 'response.finished' }
>[] => {
  const events: Exclude<
    ProviderStreamEvent,
    { readonly type: 'response.finished' }
  >[] = [];
  const usage = parseUsage(recordField(payload, 'usage'));

  if (usage !== undefined) {
    state.usage = usage;
    events.push({ type: 'usage', usage });
  }

  for (const choice of arrayField(payload, 'choices')
    .map(asRecord)
    .filter(isRecord)) {
    const delta = recordField(choice, 'delta') ?? {};
    const content = stringField(delta, 'content');
    const reasoning =
      stringField(delta, 'reasoning') ??
      stringField(delta, 'reasoning_content');
    const refusal = stringField(delta, 'refusal');

    if (content !== undefined) {
      state.text.push(content);
      events.push({ type: 'text.delta', delta: content });
    }

    if (reasoning !== undefined) {
      state.reasoning.push(reasoning);
      events.push({ type: 'reasoning.delta', delta: reasoning });
    }

    if (refusal !== undefined) {
      state.refusal.push(refusal);
      events.push({ type: 'refusal.delta', delta: refusal });
    }

    for (const call of arrayField(delta, 'tool_calls')
      .map(asRecord)
      .filter(isRecord)) {
      const index = numberField(call, 'index') ?? 0;
      const fn = recordField(call, 'function') ?? {};
      const previous = state.calls.get(index);
      const next = {
        id: stringField(call, 'id') ?? previous?.id ?? `call_${index}`,
        name: stringField(fn, 'name') ?? previous?.name ?? '',
        arguments: `${previous?.arguments ?? ''}${stringField(fn, 'arguments') ?? ''}`,
        index,
      };

      state.calls.set(index, next);
      events.push({
        type: 'tool_call.delta',
        index,
        id: next.id,
        name: next.name,
        argumentsDelta: stringField(fn, 'arguments'),
      });
    }

    state.finishReason = finishReason(choice.finish_reason);
  }

  return events;
};

const modelFromRecord = (model: Record<string, unknown>): Model => {
  const topProvider = recordField(model, 'top_provider');
  const architecture = recordField(model, 'architecture');

  return {
    id: stringField(model, 'id') ?? '',
    name: stringField(model, 'name'),
    contextWindow:
      numberField(model, 'context_length') ??
      (topProvider === undefined
        ? undefined
        : numberField(topProvider, 'context_length')) ??
      (architecture === undefined
        ? undefined
        : numberField(architecture, 'context_length')) ??
      4096,
    provider: 'openrouter',
    raw: model,
  };
};

const secret = async (
  source: string | (() => string | Promise<string>),
): Promise<string> => {
  const value = typeof source === 'function' ? await source() : source;

  if (value.trim() === '') {
    throw new ProviderErrorObject({
      provider: 'openrouter',
      code: 'auth_missing',
      message: 'OpenRouter provider requires an API key.',
    });
  }

  return value;
};

const reasoningRequest = (
  request: ProviderRequest<unknown>,
): Record<string, unknown> | undefined => {
  const value = request.flags?.reasoning;

  if (value === undefined || value === false) {
    return undefined;
  }

  return value === true
    ? {}
    : prune({ effort: value.effort, summary: value.summary });
};

const prune = (value: Record<string, unknown>): Record<string, unknown> =>
  Object.fromEntries(
    Object.entries(value).filter(([, child]) => child !== undefined),
  );

const isRecord = (
  value: Record<string, unknown> | undefined,
): value is Record<string, unknown> => value !== undefined;
