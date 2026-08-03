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
  ProviderMetadata,
  ProviderRequest,
  ProviderRerankRequest,
  ProviderRerankResult,
  ProviderStructuredFinished,
  ProviderStreamEvent,
  StructuredOutputSchema,
  StructuredOutputValue,
} from '../types/provider.js';
import { parseSseEvents } from '../utils/sse.js';
import {
  httpError,
  parseEmbedding,
  parseRerank,
  parseJsonBody,
  parseStructuredOutput,
  requireEmbeddingInput,
  requireRerankInput,
  requireRequestInput,
  streamErrorEvent,
} from './common.js';
import { authorization } from './openrouter/auth.js';
import { openRouterBody } from './openrouter/body.js';
import { modelsFromResponse } from './openrouter/models.js';
import {
  createStreamState,
  hasProviderError,
  parseFinished,
  streamEvents,
  streamFinish,
  streamToolCalls,
} from './openrouter/parse.js';

export { openRouterBody } from './openrouter/body.js';

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
  embeddings: true,
  reranking: true,
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
    const sensitiveOutput = request.flags?.sensitiveOutput === true;
    const response = await deps.transport.request({
      method: 'POST',
      url: `${baseUrl}/chat/completions`,
      headers: {
        authorization: await authorization(deps.apiKey),
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
      fields: {
        status: response.status,
        ...(sensitiveOutput ? {} : { body: response.body }),
      },
    });

    if (response.status >= 400) {
      throw httpError(
        'openrouter',
        response.status,
        response.body,
        sensitiveOutput,
      );
    }

    return parseJsonBody('openrouter', response.body, sensitiveOutput);
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
      parseFinished(await post(request, body)),
    );
  }

  return {
    metadata: openRouterMetadata,
    capabilities: openRouterCapabilities,

    complete,

    async *stream<Output = JsonValue>(
      request: ProviderRequest<Output>,
    ): AsyncIterable<ProviderStreamEvent<Output>> {
      requireRequestInput('openrouter', request);
      const sensitiveOutput = request.flags?.sensitiveOutput === true;
      const body = openRouterBody(request, true);
      const chunks = deps.transport.stream({
        method: 'POST',
        url: `${baseUrl}/chat/completions`,
        headers: {
          authorization: await authorization(deps.apiKey),
          'content-type': 'application/json',
          accept: 'text/event-stream',
        },
        body: JSON.stringify(body),
        signal: request.signal,
      });
      const state = createStreamState();

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
          payload = parseJsonBody('openrouter', event.data, sensitiveOutput);
        } catch {
          yield streamErrorEvent(
            'openrouter',
            'malformed_stream_event',
            event.data,
            undefined,
            sensitiveOutput,
          );
          return;
        }

        if (hasProviderError(payload)) {
          yield streamErrorEvent(
            'openrouter',
            'provider_error',
            'OpenRouter stream error.',
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
          'openrouter',
          request,
          streamFinish(state),
          false,
        ),
      };
    },

    async embedding(
      request: ProviderEmbeddingRequest,
    ): Promise<readonly number[]> {
      requireEmbeddingInput('openrouter', request);
      const sensitiveOutput = request.flags?.sensitiveOutput === true;
      const body = { model: request.model, input: request.input };
      await logger?.log({
        provider: 'openrouter',
        target: 'embeddings',
        event: 'http.request',
        fields: { body },
      });
      const response = await deps.transport.request({
        method: 'POST',
        url: `${baseUrl}/embeddings`,
        headers: {
          authorization: await authorization(deps.apiKey),
          'content-type': 'application/json',
          accept: 'application/json',
        },
        body: JSON.stringify(body),
        signal: request.signal,
      });
      await logger?.log({
        provider: 'openrouter',
        target: 'embeddings',
        event: 'http.response',
        fields: {
          status: response.status,
          ...(sensitiveOutput ? {} : { body: response.body }),
        },
      });

      if (response.status >= 400) {
        throw httpError(
          'openrouter',
          response.status,
          response.body,
          sensitiveOutput,
        );
      }

      return parseEmbedding(
        'openrouter',
        parseJsonBody('openrouter', response.body, sensitiveOutput),
      );
    },

    async rerank(
      request: ProviderRerankRequest,
    ): Promise<readonly ProviderRerankResult[]> {
      requireRerankInput('openrouter', request);
      const sensitiveOutput = request.flags?.sensitiveOutput === true;
      const body = {
        model: request.model,
        query: request.query,
        documents: request.documents,
        ...(request.topN === undefined ? {} : { top_n: request.topN }),
      };
      await logger?.log({
        provider: 'openrouter',
        target: 'rerank',
        event: 'http.request',
        fields: { body },
      });
      const response = await deps.transport.request({
        method: 'POST',
        url: `${baseUrl}/rerank`,
        headers: {
          authorization: await authorization(deps.apiKey),
          'content-type': 'application/json',
          accept: 'application/json',
        },
        body: JSON.stringify(body),
        signal: request.signal,
      });
      await logger?.log({
        provider: 'openrouter',
        target: 'rerank',
        event: 'http.response',
        fields: {
          status: response.status,
          ...(sensitiveOutput ? {} : { body: response.body }),
        },
      });

      if (response.status >= 400) {
        throw httpError(
          'openrouter',
          response.status,
          response.body,
          sensitiveOutput,
        );
      }

      return parseRerank(
        'openrouter',
        parseJsonBody('openrouter', response.body, sensitiveOutput),
      );
    },

    async models(signal?: AbortSignal): Promise<readonly Model[]> {
      const response = await deps.transport.request({
        method: 'GET',
        url: `${baseUrl}/models`,
        headers: {
          authorization: await authorization(deps.apiKey),
          accept: 'application/json',
        },
        signal,
      });

      if (response.status >= 400) {
        throw httpError('openrouter', response.status, response.body);
      }

      return modelsFromResponse(parseJsonBody('openrouter', response.body));
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
