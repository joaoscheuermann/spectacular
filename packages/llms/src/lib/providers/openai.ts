import { ProviderErrorObject } from '../classes/provider-error.js';
import type { LlmDebugLogger } from '../debug.js';
import type { HttpTransport } from '../types/http.js';
import type {
  JsonValue,
  LlmProvider,
  Model,
  ProviderCapabilities,
  ProviderFinished,
  ProviderId,
  ProviderMetadata,
  ProviderRequest,
  ProviderStreamEvent,
  ProviderToolCall,
} from '../types/provider.js';
import {
  asRecord,
  arrayField,
  recordField,
  stringField,
} from '../utils/json.js';
import {
  httpError,
  parseJsonBody,
  requireRequestInput,
  streamErrorEvent,
} from './common.js';
import { authorization, type SecretSource } from './openai/auth.js';
import { openAiBody } from './openai/body.js';
import { parseStructuredOutputWithDebug } from './openai/debug.js';
import {
  createTextSnapshots,
  parseFinished,
  recordTextSnapshot,
  streamEvent,
  streamText,
} from './openai/parse.js';
import { parseSseEvents } from '../utils/sse.js';

export type { SecretSource } from './openai/auth.js';
export { openAiBody } from './openai/body.js';

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

    await logger?.log({
      provider: debugProvider,
      target: 'responses',
      event: 'http.response',
      fields: {
        status: response.status,
        ...(sensitiveOutput ? {} : { body: response.body }),
      },
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

      return parseStructuredOutputWithDebug(
        logger,
        debugProvider,
        request,
        parseFinished(await send(request, body)),
        'complete',
      );
    },

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
          await logger?.log({
            provider: debugProvider,
            target: 'responses',
            event: 'stream.event.invalid_json',
            fields: sensitiveOutput ? {} : { data: event.data },
          });
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
          await logger?.log({
            provider: debugProvider,
            target: 'responses',
            event: 'stream.response.completed',
            fields: sensitiveOutput ? {} : { payload },
          });
          const response = recordField(payload, 'response') ?? payload;
          const finish = await parseStructuredOutputWithDebug(
            logger,
            debugProvider,
            request,
            parseFinished(
              response,
              streamText(text, textSnapshots),
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
            fields: sensitiveOutput ? {} : { payload },
          });
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
        finish: await parseStructuredOutputWithDebug(
          logger,
          debugProvider,
          request,
          {
            text: streamText(text, textSnapshots) ?? '',
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
