import { ProviderErrorObject } from '../classes/provider-error.js';
import type { LlmDebugLogger } from '../debug.js';
import type { HttpTransport } from '../types/http.js';
import type {
  JsonObject,
  LlmProvider,
  Model,
  ProviderCapabilities,
  ProviderFinished,
  ProviderMetadata,
  ProviderRequest,
  ProviderStreamEvent,
  ProviderToolCall,
  ReasoningMetadata,
} from '../types/provider.js';
import { asRecord, arrayField, recordField, stringField } from '../utils/json.js';
import { parseSseEvents } from '../utils/sse.js';
import {
  finishReason,
  httpError,
  messageText,
  parseJsonBody,
  parseUsage,
  requireRequestInput,
  streamErrorEvent,
} from './common.js';

export type SecretSource = string | (() => string | Promise<string>);

export type OpenAiProviderDeps = {
  readonly transport: HttpTransport;
  readonly apiKey?: SecretSource;
  readonly authorization?: SecretSource;
  readonly baseUrl?: string;
  readonly debugLogger?: LlmDebugLogger;
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
};

export const createOpenAiProvider = (deps: OpenAiProviderDeps): LlmProvider => {
  const baseUrl = deps.baseUrl ?? openAiMetadata.baseUrl;
  const logger = deps.debugLogger;

  const send = async (
    request: ProviderRequest,
    body: Record<string, unknown>,
  ): Promise<Record<string, unknown>> => {
    const auth = await authorization(deps);
    const response = await deps.transport.request({
      method: 'POST',
      url: `${baseUrl}/responses`,
      headers: {
        authorization: auth,
        'content-type': 'application/json',
        accept: 'application/json',
      },
      body: JSON.stringify(body),
      signal: request.signal,
    });

    await logger?.log({
      provider: 'openai',
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

    async complete(request: ProviderRequest): Promise<ProviderFinished> {
      requireRequestInput('openai', request);
      const body = openAiBody(request, false);
      await logger?.log({
        provider: 'openai',
        target: 'responses',
        event: 'http.request',
        fields: { body },
      });

      return parseOpenAiFinished(await send(request, body));
    },

    async *stream(request: ProviderRequest): AsyncIterable<ProviderStreamEvent> {
      requireRequestInput('openai', request);
      const body = openAiBody(request, true);
      const auth = await authorization(deps);
      const text: string[] = [];
      const reasoning: string[] = [];
      const calls = new Map<number, ProviderToolCall>();

      yield { type: 'response.started', provider: 'openai', model: body.model as string };

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
          yield streamErrorEvent('openai', 'malformed_stream_event', event.data);
          return;
        }

        const parsed = openAiStreamEvent(payload, calls);

        if (parsed?.type === 'text.delta') {
          text.push(parsed.delta);
        }

        if (parsed?.type === 'reasoning.delta') {
          reasoning.push(parsed.delta);
        }

        if (parsed !== undefined) {
          yield parsed;
        }

        if (payload.type === 'response.completed') {
          const response = recordField(payload, 'response') ?? payload;
          const finish = parseOpenAiFinished(response, text.join(''), reasoning.join(''));
          const usage = finish.usage;

          if (usage !== undefined) {
            yield { type: 'usage', usage };
          }

          yield { type: 'response.finished', finish };
          return;
        }

        if (payload.type === 'response.failed') {
          yield streamErrorEvent('openai', 'provider_error', 'OpenAI stream failed.');
          return;
        }
      }

      yield {
        type: 'response.finished',
        finish: {
          text: text.join(''),
          reasoning: { text: reasoning.join('') },
          finishReason: 'unknown',
          toolCalls: [...calls.values()],
        },
      };
    },

    async models(signal?: AbortSignal): Promise<readonly Model[]> {
      const auth = await authorization(deps);
      const response = await deps.transport.request({
        method: 'GET',
        url: `${baseUrl}/models`,
        headers: { authorization: auth, accept: 'application/json' },
        signal,
      });

      if (response.status >= 400) {
        throw httpError('openai', response.status, response.body);
      }

      return arrayField(parseJsonBody('openai', response.body), 'data')
        .map(asRecord)
        .filter((model): model is Record<string, unknown> => model !== undefined)
        .map((model) => ({
          id: stringField(model, 'id') ?? '',
          name: stringField(model, 'id'),
          provider: 'openai',
          raw: model,
        }))
        .filter((model) => model.id !== '');
    },

    async validateModel(model: string, signal?: AbortSignal): Promise<Model> {
      const found = (await this.models(signal)).find((item) => item.id === model);

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
  request: ProviderRequest,
  stream: boolean,
): Record<string, unknown> => {
  requireRequestInput('openai', request);
  const alias = fastAlias(request.model);
  const system = request.messages
    .filter((message) => message.role === 'system')
    .map(messageText)
    .filter((text) => text !== '')
    .join('\n\n');
  const nonSystem = request.messages.filter((message) => message.role !== 'system');
  const reasoning = reasoningRequest(request);

  return prune({
    model: alias.model,
    instructions: system === '' ? undefined : system,
    input: nonSystem.map((message) => {
      if (message.role === 'tool') {
        return {
          type: 'function_call_output',
          call_id: message.toolCallId,
          output: messageText(message),
        };
      }

      return {
        role: message.role,
        content: [{ type: message.role === 'assistant' ? 'output_text' : 'input_text', text: messageText(message) }],
      };
    }),
    tools: request.tools?.map((tool) => ({
      type: 'function',
      name: tool.name,
      description: tool.description,
      parameters: tool.inputSchema,
      strict: tool.strict,
    })),
    temperature: request.temperature,
    max_output_tokens: request.maxOutputTokens,
    service_tier: request.flags?.serviceTier ?? alias.serviceTier,
    reasoning,
    stream,
  });
};

const openAiStreamEvent = (
  payload: Record<string, unknown>,
  calls: Map<number, ProviderToolCall>,
): ProviderStreamEvent | undefined => {
  const type = stringField(payload, 'type');

  if (type === 'response.output_text.delta') {
    return { type: 'text.delta', delta: stringField(payload, 'delta') ?? '' };
  }

  if (
    type === 'response.reasoning_summary_text.delta' ||
    type === 'response.reasoning_text.delta'
  ) {
    return { type: 'reasoning.delta', delta: stringField(payload, 'delta') ?? '' };
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
      id: stringField(item, 'call_id') ?? stringField(item, 'id') ?? `call_${index}`,
      name: stringField(item, 'name') ?? '',
      arguments: stringField(item, 'arguments') ?? calls.get(index)?.arguments ?? '',
      index,
    };

    calls.set(index, call);

    return { type: 'tool_call.done', call };
  }

  return undefined;
};

const parseOpenAiFinished = (
  response: Record<string, unknown>,
  streamText?: string,
  streamReasoning?: string,
): ProviderFinished => {
  const output = arrayField(response, 'output').map(asRecord).filter(isRecord);
  const outputText =
    stringField(response, 'output_text') ??
    streamText ??
    output
      .flatMap((item) => arrayField(item, 'content'))
      .map(asRecord)
      .filter(isRecord)
      .filter((content) => content.type === 'output_text')
      .map((content) => stringField(content, 'text') ?? '')
      .join('');
  const toolCalls = output
    .filter((item) => item.type === 'function_call')
    .map((item, index) => ({
      id: stringField(item, 'call_id') ?? stringField(item, 'id') ?? `call_${index}`,
      name: stringField(item, 'name') ?? '',
      arguments: stringField(item, 'arguments') ?? '',
      index,
    }));
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
    finishReason: finishReason(response.status === 'completed' ? 'stop' : response.status),
    usage,
    reasoning,
    toolCalls,
  };
};

const authorization = async (
  deps: OpenAiProviderDeps,
): Promise<string> => {
  if (deps.apiKey !== undefined && deps.authorization !== undefined) {
    throw new ProviderErrorObject({
      provider: 'openai',
      code: 'auth_ambiguous',
      message: 'OpenAI provider accepts either apiKey or authorization, not both.',
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

const reasoningRequest = (request: ProviderRequest): JsonObject | undefined => {
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

const prune = (value: Record<string, unknown>): Record<string, unknown> =>
  Object.fromEntries(
    Object.entries(value).filter(([, child]) => child !== undefined),
  );

const isRecord = (value: Record<string, unknown> | undefined): value is Record<string, unknown> =>
  value !== undefined;
