import type {
  JsonValue,
  LlmProvider,
  ProviderFinished,
  ProviderRequest,
  ProviderStreamEvent,
} from 'llms';

type Output = (
  system: string,
  input: string,
  index: number,
) => unknown | Promise<unknown>;

export type ProviderFake = {
  readonly provider: LlmProvider;
  readonly requests: readonly ProviderRequest<unknown>[];
};

export const createProvider = (
  output: Output = defaultOutput,
): ProviderFake => {
  const requests: ProviderRequest<unknown>[] = [];

  return {
    requests,
    provider: {
      metadata: {
        id: 'fake',
        name: 'Fake',
        baseUrl: 'https://fake.invalid',
      },
      capabilities: {
        streaming: false,
        tools: false,
        reasoning: true,
        modelListing: false,
        oauth: false,
        serviceTier: false,
        structuredOutputs: true,
      },
      complete: async <Result = JsonValue>(
        request: ProviderRequest<Result>,
      ) => {
        const index = requests.length;
        requests.push(request);
        const system = text(request, 'system');
        const input = text(request, 'user');
        const structured = await output(system, input, index);

        return finish(structured) as ProviderFinished<Result>;
      },
      stream: async function* <Result = JsonValue>() {
        yield* [] as ProviderStreamEvent<Result>[];
      },
      models: async () => [],
      validateModel: async (model) => ({ id: model }),
    },
  };
};

const text = (
  request: ProviderRequest<unknown>,
  role: 'system' | 'user',
): string => {
  const content = request.messages.find(
    (message) => message.role === role,
  )?.content;
  if (typeof content !== 'string') throw new Error(`Missing ${role} message`);
  return content;
};

const defaultOutput: Output = (system, input) => {
  const evidence = JSON.parse(input) as { readonly path: string };

  if (system.startsWith('Classify one repository file')) {
    return { type: 'documentation' };
  }

  if (system.startsWith('Generate OKF metadata')) {
    return {
      type: 'Documentation',
      title: evidence.path,
      description: `Documents ${evidence.path}.`,
      tags: ['documentation'],
    };
  }

  return {
    summary:
      '# Subject\n- Repository documentation.\n# Instructions And Decisions\n- Not present in the input.\n# References\n- Not present in the input.\n# Constraints And Open Questions\n- Not present in the input.',
  };
};

const finish = (structured: unknown): ProviderFinished<unknown> => ({
  text: structured === undefined ? '' : JSON.stringify(structured),
  finishReason: 'stop',
  toolCalls: [],
  structured,
});
