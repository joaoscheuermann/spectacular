import { ProviderErrorObject } from '../classes/provider-error.js';
import type { LlmDebugLogger } from '../debug.js';
import type { HttpTransport } from '../types/http.js';
import type {
  JsonValue,
  LlmProvider,
  Model,
  ProviderCapabilities,
  ProviderEmbeddingRequest,
  ProviderFinished,
  ProviderMessage,
  ProviderMetadata,
  ProviderRequest,
  ProviderRerankRequest,
  ProviderRerankResult,
  ProviderStructuredFinished,
  ProviderStreamEvent,
  ProviderToolCall,
  StructuredOutputSchema,
  StructuredOutputValue,
} from '../types/provider.js';
import { asRecord, arrayField, stringField } from '../utils/json.js';
import { parseSseEvents } from '../utils/sse.js';
import {
  httpError,
  parseEmbedding,
  parseRerank,
  messagesWithStructuredSchema,
  messageText,
  parseJsonBody,
  parseStructuredOutput,
  requireEmbeddingInput,
  requireRerankInput,
  requestReasoningEffort,
  requireRequestInput,
  streamErrorEvent,
  structuredJsonSchema,
} from './common.js';
import {
  createStreamState,
  hasProviderError,
  parseFinished,
  streamEvents,
  streamFinish,
  streamToolCalls,
} from './openrouter/parse.js';

type SecretSource = string | (() => string | Promise<string>);
type DebugFields = Readonly<Record<string, unknown>>;
type DebugLog = (
  target: string,
  event: string,
  fields?: DebugFields,
) => Promise<void>;

export type LmStudioOpenAiProviderDeps = {
  readonly transport: HttpTransport;
  readonly apiKey?: SecretSource;
  readonly authorization?: SecretSource;
  readonly baseUrl?: string;
  readonly debugLogger?: LlmDebugLogger;
};

export const lmStudioOpenAiMetadata: ProviderMetadata = {
  id: 'lmstudio-openai',
  name: 'LM Studio OpenAI Compatibility',
  baseUrl: 'http://localhost:1234/v1',
};

export const lmStudioOpenAiCapabilities: ProviderCapabilities = {
  streaming: true,
  embeddings: true,
  reranking: true,
  tools: true,
  reasoning: true,
  modelListing: true,
  oauth: false,
  serviceTier: false,
  structuredOutputs: true,
};

export const createLmStudioOpenAiProvider = (
  deps: LmStudioOpenAiProviderDeps,
): LlmProvider => {
  const baseUrl = deps.baseUrl ?? lmStudioOpenAiMetadata.baseUrl;

  const logger = deps.debugLogger;

  const log: DebugLog = async (target, event, fields) => {
    await logger?.log({ provider: 'lmstudio-openai', target, event, fields });
  };

  const post = async (
    request: ProviderRequest<unknown>,
    body: Record<string, unknown>,
  ): Promise<Record<string, unknown>> => {
    const sensitiveOutput = request.flags?.sensitiveOutput === true;
    const response = await deps.transport.request({
      method: 'POST',
      url: `${baseUrl}/chat/completions`,
      headers: {
        ...(await authHeader(deps)),
        'content-type': 'application/json',
        accept: 'application/json',
      },
      body: JSON.stringify(body),
      signal: request.signal,
    });

    await log('chat/completions', 'http.response', {
      status: response.status,
      ...(sensitiveOutput ? {} : { body: response.body }),
    });

    if (response.status >= 400) {
      throw httpError(
        'lmstudio-openai',
        response.status,
        response.body,
        sensitiveOutput,
      );
    }

    return parseJsonBody('lmstudio-openai', response.body, sensitiveOutput);
  };

  async function complete<Schema extends StructuredOutputSchema>(
    request: ProviderRequest<StructuredOutputValue<Schema>, Schema> & {
      readonly schema: Schema;
    },
  ): Promise<ProviderStructuredFinished<StructuredOutputValue<Schema>>>;
  async function complete<Output = JsonValue>(
    request: ProviderRequest<Output>,
  ): Promise<ProviderFinished<Output>>;
  async function complete<Output = JsonValue>(
    request: ProviderRequest<Output>,
  ): Promise<ProviderFinished<Output>> {
    requireRequestInput('lmstudio-openai', request);
    const body = chatBody(request, false);
    await log('chat/completions', 'http.request', { body });

    return parseStructuredOutput(
      'lmstudio-openai',
      request,
      parseFinished(await post(request, body)),
    );
  }

  return {
    metadata: lmStudioOpenAiMetadata,
    capabilities: lmStudioOpenAiCapabilities,

    complete,

    async *stream<Output = JsonValue>(
      request: ProviderRequest<Output>,
    ): AsyncIterable<ProviderStreamEvent<Output>> {
      requireRequestInput('lmstudio-openai', request);
      const sensitiveOutput = request.flags?.sensitiveOutput === true;
      const body = chatBody(request, true);
      const state = createStreamState();
      await log('chat/completions', 'http.request', { body });
      const auth = await authHeader(deps);

      yield {
        type: 'response.started',
        provider: 'lmstudio-openai',
        model: request.model,
      };

      for await (const event of parseSseEvents(
        deps.transport.stream({
          method: 'POST',
          url: `${baseUrl}/chat/completions`,
          headers: {
            ...auth,
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
          payload = parseJsonBody(
            'lmstudio-openai',
            event.data,
            sensitiveOutput,
          );
        } catch {
          await log('chat/completions', 'stream.event.invalid_json', {
            ...(sensitiveOutput ? {} : { data: event.data }),
          });
          yield streamErrorEvent(
            'lmstudio-openai',
            'malformed_stream_event',
            event.data,
            undefined,
            sensitiveOutput,
          );
          return;
        }

        if (hasProviderError(payload)) {
          await log('chat/completions', 'stream.provider_error', {
            ...(sensitiveOutput ? {} : { data: event.data, payload }),
          });
          yield streamErrorEvent(
            'lmstudio-openai',
            'provider_error',
            'LM Studio OpenAI-compatible stream error.',
            event.data,
            sensitiveOutput,
          );
          return;
        }

        for (const parsed of streamEvents(payload, state)) {
          yield parsed;
        }
      }

      for (const call of streamToolCalls(state)) {
        yield { type: 'tool_call.done', call };
      }

      yield {
        type: 'response.finished',
        finish: parseStructuredOutput(
          'lmstudio-openai',
          request,
          streamFinish(state),
          false,
        ),
      };
    },

    async embedding(
      request: ProviderEmbeddingRequest,
    ): Promise<readonly number[]> {
      requireEmbeddingInput('lmstudio-openai', request);
      const sensitiveOutput = request.flags?.sensitiveOutput === true;
      const body = { model: request.model, input: request.input };
      await log('embeddings', 'http.request', { body });
      const response = await deps.transport.request({
        method: 'POST',
        url: `${baseUrl}/embeddings`,
        headers: {
          ...(await authHeader(deps)),
          'content-type': 'application/json',
          accept: 'application/json',
        },
        body: JSON.stringify(body),
        signal: request.signal,
      });
      await log('embeddings', 'http.response', {
        status: response.status,
        ...(sensitiveOutput ? {} : { body: response.body }),
      });

      if (response.status >= 400) {
        throw httpError(
          'lmstudio-openai',
          response.status,
          response.body,
          sensitiveOutput,
        );
      }

      return parseEmbedding(
        'lmstudio-openai',
        parseJsonBody('lmstudio-openai', response.body, sensitiveOutput),
      );
    },

    async rerank(
      request: ProviderRerankRequest,
    ): Promise<readonly ProviderRerankResult[]> {
      requireRerankInput('lmstudio-openai', request);
      const sensitiveOutput = request.flags?.sensitiveOutput === true;
      const body = {
        model: request.model,
        query: request.query,
        documents: request.documents,
        ...(request.topN === undefined ? {} : { top_n: request.topN }),
      };
      await log('rerank', 'http.request', { body });
      const response = await deps.transport.request({
        method: 'POST',
        url: `${baseUrl}/rerank`,
        headers: {
          ...(await authHeader(deps)),
          'content-type': 'application/json',
          accept: 'application/json',
        },
        body: JSON.stringify(body),
        signal: request.signal,
      });
      await log('rerank', 'http.response', {
        status: response.status,
        ...(sensitiveOutput ? {} : { body: response.body }),
      });

      if (response.status >= 400) {
        throw httpError(
          'lmstudio-openai',
          response.status,
          response.body,
          sensitiveOutput,
        );
      }

      return parseRerank(
        'lmstudio-openai',
        parseJsonBody('lmstudio-openai', response.body, sensitiveOutput),
      );
    },

    async models(signal?: AbortSignal): Promise<readonly Model[]> {
      await log('models', 'http.request', { method: 'GET' });
      const response = await deps.transport.request({
        method: 'GET',
        url: `${baseUrl}/models`,
        headers: {
          ...(await authHeader(deps)),
          accept: 'application/json',
        },
        signal,
      });

      await log('models', 'http.response', {
        status: response.status,
        body: response.body,
      });

      if (response.status >= 400) {
        throw httpError('lmstudio-openai', response.status, response.body);
      }

      return models(parseJsonBody('lmstudio-openai', response.body));
    },

    async validateModel(model: string, signal?: AbortSignal): Promise<Model> {
      const found = (await this.models(signal)).find(
        (item) => item.id === model,
      );

      if (found === undefined) {
        throw new ProviderErrorObject({
          provider: 'lmstudio-openai',
          code: 'missing_model',
          message: `LM Studio OpenAI-compatible model is not available: ${model}`,
        });
      }

      return found;
    },
  };
};

const chatBody = (
  request: ProviderRequest<unknown>,
  stream: boolean,
): Record<string, unknown> => {
  assertStructuredToolsSupported(request);

  const schema = structuredJsonSchema('lmstudio-openai', request.schema);
  const messages = messagesWithStructuredSchema(
    'lmstudio-openai',
    request,
    schema,
  );
  const tools = chatTools(request);

  return prune({
    model: request.model,
    messages: messages.map(chatMessage),
    tools,
    temperature: request.temperature,
    max_tokens: request.maxOutputTokens,
    reasoning_effort: requestReasoningEffort(request),
    response_format:
      schema === undefined
        ? undefined
        : {
            type: 'json_schema',
            json_schema: {
              name: 'structured_output',
              strict: true,
              schema,
            },
          },
    stream,
  });
};

const chatTools = (
  request: ProviderRequest<unknown>,
): readonly Record<string, unknown>[] | undefined => {
  const userTools = request.tools?.map(chatTool) ?? [];

  return userTools.length === 0 ? undefined : userTools;
};

const chatTool = (
  tool: NonNullable<ProviderRequest<unknown>['tools']>[number],
): Record<string, unknown> => ({
  type: 'function',
  function: prune({
    name: tool.name,
    description: tool.description,
    parameters: tool.inputSchema,
  }),
});

const assertStructuredToolsSupported = (
  request: ProviderRequest<unknown>,
): void => {
  if (request.schema === undefined || !hasTools(request)) {
    return;
  }

  throw new ProviderErrorObject({
    provider: 'lmstudio-openai',
    code: 'unsupported_structured_tools',
    message:
      'LM Studio OpenAI-compatible provider does not support requests that combine tools with structured output.',
  });
};

const chatMessage = (message: ProviderMessage): Record<string, unknown> =>
  prune({
    role: message.role,
    content: messageText(message),
    tool_call_id: message.toolCallId,
    tool_calls: message.toolCalls?.map(chatToolCall),
    name: message.name,
  });

const chatToolCall = (call: ProviderToolCall): Record<string, unknown> => ({
  id: call.id,
  type: 'function',
  function: { name: call.name, arguments: call.arguments },
});

const hasTools = (request: ProviderRequest<unknown>): boolean =>
  (request.tools?.length ?? 0) > 0;

const models = (body: Record<string, unknown>): readonly Model[] =>
  arrayField(body, 'data')
    .map(asRecord)
    .filter(isRecord)
    .map((model) => ({
      id: stringField(model, 'id') ?? '',
      name: stringField(model, 'id'),
      provider: 'lmstudio-openai',
      raw: model,
    }))
    .filter((model) => model.id !== '');

const authHeader = async (
  deps: LmStudioOpenAiProviderDeps,
): Promise<Record<string, string>> => {
  const value = await authorization(deps);

  return value === undefined ? {} : { authorization: value };
};

const authorization = async (
  deps: LmStudioOpenAiProviderDeps,
): Promise<string | undefined> => {
  const apiKey = await secret(deps.apiKey);
  const auth = await secret(deps.authorization);

  if (apiKey !== undefined && auth !== undefined) {
    throw new ProviderErrorObject({
      provider: 'lmstudio-openai',
      code: 'auth_ambiguous',
      message:
        'LM Studio OpenAI-compatible provider accepts either apiKey or authorization, not both.',
    });
  }

  if (apiKey !== undefined) {
    return `Bearer ${apiKey}`;
  }

  return auth;
};

const secret = async (
  source: SecretSource | undefined,
): Promise<string | undefined> => {
  if (source === undefined) {
    return undefined;
  }

  const value = typeof source === 'function' ? await source() : source;

  return value.trim() === '' ? undefined : value;
};

const prune = (value: Record<string, unknown>): Record<string, unknown> =>
  Object.fromEntries(
    Object.entries(value).filter(([, child]) => child !== undefined),
  );

const isRecord = (
  value: Record<string, unknown> | undefined,
): value is Record<string, unknown> => value !== undefined;
