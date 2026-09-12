import type {
  JsonValue,
  LlmProvider,
  ProviderFinished,
  ProviderRequest,
  ProviderStreamEvent,
} from 'llms';

export interface FakeProvider {
  readonly provider: LlmProvider;
  readonly requests: ProviderRequest<unknown>[];
}

export const finish = (
  text: string,
  toolCalls: ProviderFinished['toolCalls'] = [],
): ProviderFinished => ({
  text,
  finishReason: toolCalls.length > 0 ? 'tool_calls' : 'stop',
  toolCalls,
});

/** Creates a local provider that records requests and never uses the network. */
export const fakeProvider = (
  respond: (request: ProviderRequest<unknown>) => ProviderFinished = () =>
    finish('done'),
): FakeProvider => {
  const requests: ProviderRequest<unknown>[] = [];

  const provider = {
    metadata: { id: 'fake', name: 'Fake', baseUrl: 'https://fake.invalid' },
    capabilities: {
      streaming: true,
      embeddings: true,
      reranking: true,
      tools: true,
      reasoning: true,
      modelListing: true,
      oauth: false,
      serviceTier: false,
      structuredOutputs: true,
    },
    complete: async <Output = JsonValue>(request: ProviderRequest<Output>) => {
      requests.push(request);

      return respond(request) as ProviderFinished<Output>;
    },
    stream: async function* <Output = JsonValue>(
      request: ProviderRequest<Output>,
    ): AsyncIterable<ProviderStreamEvent<Output>> {
      requests.push(request);

      const response = respond(request) as ProviderFinished<Output>;

      yield {
        type: 'response.started',
        provider: 'fake',
        model: request.model,
      };

      if (response.text.length > 0) {
        yield { type: 'text.delta', delta: response.text };
      }

      yield { type: 'reasoning.delta', delta: 'private reasoning' };

      yield { type: 'response.finished', finish: response };
    },
    embedding: async () => ({ embedding: [] }),
    rerank: async () => ({ results: [] }),
    models: async () => [],
    validateModel: async (model: string) => ({ id: model }),
  } as LlmProvider;

  return { provider, requests };
};
