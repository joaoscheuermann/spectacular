import type { Logger } from 'pino';

import { ProviderErrorObject } from '../classes/provider-error.js';
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
  ProviderToolCall,
  StructuredOutputSchema,
  StructuredOutputValue,
} from '../types/provider.js';
import {
  asRecord,
  arrayField,
  recordField,
  stringField,
} from '../utils/json.js';
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
import { authorization, type SecretSource } from './openai/auth.js';
import { openAiBody } from './openai/body.js';
import {
  createTextSnapshots,
  parseFinished,
  recordTextSnapshot,
  streamEvent,
  streamText,
} from './openai/parse.js';
import { parseSseEvents } from '../utils/sse.js';
import { withProviderLogging } from './logging.js';

export type { SecretSource } from './openai/auth.js';
export { openAiBody } from './openai/body.js';

export type OpenAiProviderDeps = {
  readonly transport: HttpTransport;
  readonly apiKey?: SecretSource;
  readonly authorization?: SecretSource;
  readonly baseUrl?: string;
  readonly logger: Logger;
};

export const openAiMetadata: ProviderMetadata = {
  id: 'openai',
  name: 'OpenAI',
  baseUrl: 'https://api.openai.com/v1',
};

export const openAiCapabilities: ProviderCapabilities = {
  streaming: true,
  embeddings: true,
  reranking: true,
  tools: true,
  reasoning: true,
  modelListing: true,
  oauth: false,
  serviceTier: true,
  structuredOutputs: true,
};

export const createOpenAiProvider = (deps: OpenAiProviderDeps): LlmProvider =>
  withProviderLogging(createOpenAiProviderCore(deps), deps.logger);

export const createOpenAiProviderCore = (
  deps: Omit<OpenAiProviderDeps, 'logger'>,
): LlmProvider => {
  const baseUrl = deps.baseUrl ?? openAiMetadata.baseUrl;

  const send = async (
    request: ProviderRequest<unknown>,
    body: Record<string, unknown>,
  ): Promise<Record<string, unknown>> => {
    const auth = await authorization(deps);
    const sensitiveOutput = request.flags?.sensitiveOutput === true;

    const response = await deps.transport.request({
      method: 'POST',
      url: `${baseUrl}/responses`,
      headers: {
        ...(auth === undefined ? {} : { authorization: auth }),
        'content-type': 'application/json',
        accept: 'application/json',
      },
      body: JSON.stringify(body),
      signal: request.signal,
    });

    if (response.status >= 400) {
      throw httpError(
        'openai',
        response.status,
        response.body,
        sensitiveOutput,
      );
    }

    return parseJsonBody('openai', response.body, sensitiveOutput);
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
    requireRequestInput('openai', request);
    const body = openAiBody(request, false);
    return parseStructuredOutput(
      'openai',
      request,
      parseFinished(await send(request, body)),
    );
  }

  return {
    metadata: openAiMetadata,
    capabilities: openAiCapabilities,

    complete,

    async *stream<Output = JsonValue>(
      request: ProviderRequest<Output>,
    ): AsyncIterable<ProviderStreamEvent<Output>> {
      requireRequestInput('openai', request);
      const sensitiveOutput = request.flags?.sensitiveOutput === true;
      const body = openAiBody(request, true);
      const text: string[] = [];
      const reasoning: string[] = [];
      const refusals: string[] = [];
      const textSnapshots = createTextSnapshots();
      const calls = new Map<number, ProviderToolCall>();
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
            ...(auth === undefined ? {} : { authorization: auth }),
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
          payload = parseJsonBody('openai', event.data, sensitiveOutput);
        } catch {
          yield streamErrorEvent(
            'openai',
            'malformed_stream_event',
            event.data,
            undefined,
            sensitiveOutput,
          );
          return;
        }

        const parsed = streamEvent(payload, calls);
        recordTextSnapshot(textSnapshots, payload);

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
          const response = recordField(payload, 'response') ?? payload;
          const finish = parseStructuredOutput(
            'openai',
            request,
            parseFinished(
              response,
              streamText(text, textSnapshots),
              reasoning.join(''),
              refusals.join('') || undefined,
              [...calls.values()],
            ),
            false,
          );
          const usage = finish.usage;

          if (usage !== undefined) {
            yield { type: 'usage', usage };
          }

          yield { type: 'response.finished', finish };
          return;
        }

        if (payload.type === 'response.failed') {
          yield streamErrorEvent(
            'openai',
            'provider_error',
            'OpenAI stream failed.',
            undefined,
            sensitiveOutput,
          );
          return;
        }
      }

      yield {
        type: 'response.finished',
        finish: parseStructuredOutput(
          'openai',
          request,
          {
            text: streamText(text, textSnapshots) ?? '',
            reasoning: { text: reasoning.join('') },
            refusal: refusals.join('') || undefined,
            finishReason: 'unknown',
            toolCalls: [...calls.values()],
          },
          false,
        ),
      };
    },

    async embedding(
      request: ProviderEmbeddingRequest,
    ): Promise<readonly number[]> {
      requireEmbeddingInput('openai', request);
      const auth = await authorization(deps);
      const sensitiveOutput = request.flags?.sensitiveOutput === true;
      const body = {
        model: request.model,
        input: request.input,
        ...(request.dimensions === undefined
          ? {}
          : { dimensions: request.dimensions }),
      };
      const response = await deps.transport.request({
        method: 'POST',
        url: `${baseUrl}/embeddings`,
        headers: {
          ...(auth === undefined ? {} : { authorization: auth }),
          'content-type': 'application/json',
          accept: 'application/json',
        },
        body: JSON.stringify(body),
        signal: request.signal,
      });
      if (response.status >= 400) {
        throw httpError(
          'openai',
          response.status,
          response.body,
          sensitiveOutput,
        );
      }

      return parseEmbedding(
        'openai',
        parseJsonBody('openai', response.body, sensitiveOutput),
      );
    },

    async rerank(
      request: ProviderRerankRequest,
    ): Promise<readonly ProviderRerankResult[]> {
      requireRerankInput('openai', request);
      const auth = await authorization(deps);
      const sensitiveOutput = request.flags?.sensitiveOutput === true;
      const body = {
        model: request.model,
        query: request.query,
        documents: request.documents,
        ...(request.topN === undefined ? {} : { top_n: request.topN }),
      };
      const response = await deps.transport.request({
        method: 'POST',
        url: `${baseUrl}/rerank`,
        headers: {
          ...(auth === undefined ? {} : { authorization: auth }),
          'content-type': 'application/json',
          accept: 'application/json',
        },
        body: JSON.stringify(body),
        signal: request.signal,
      });
      if (response.status >= 400) {
        throw httpError(
          'openai',
          response.status,
          response.body,
          sensitiveOutput,
        );
      }

      return parseRerank(
        'openai',
        parseJsonBody('openai', response.body, sensitiveOutput),
      );
    },

    async models(signal?: AbortSignal): Promise<readonly Model[]> {
      const auth = await authorization(deps);
      const response = await deps.transport.request({
        method: 'GET',
        url: `${baseUrl}/models`,
        headers: {
          ...(auth === undefined ? {} : { authorization: auth }),
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
