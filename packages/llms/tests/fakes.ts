import type {
  HttpRequest,
  HttpResponse,
  HttpStreamChunk,
  HttpTransport,
} from '../src/index.js';

export type FakeTransport = HttpTransport & {
  readonly requests: readonly HttpRequest[];
};

export const response = (
  body: unknown,
  status = 200,
  headers: Record<string, string> = {},
): HttpResponse => ({
  status,
  headers,
  body: typeof body === 'string' ? body : JSON.stringify(body),
});

export const fakeTransport = (options: {
  readonly responses?: readonly HttpResponse[];
  readonly streams?: readonly (readonly HttpStreamChunk[])[];
}): FakeTransport => {
  const requests: HttpRequest[] = [];
  const responses = [...(options.responses ?? [])];
  const streams = [...(options.streams ?? [])];

  return {
    requests,

    async request(request: HttpRequest): Promise<HttpResponse> {
      requests.push(request);
      const next = responses.shift();

      if (next === undefined) {
        throw new Error('Fake transport response was not configured.');
      }

      return next;
    },

    async *stream(request: HttpRequest): AsyncIterable<HttpStreamChunk> {
      requests.push(request);
      const next = streams.shift();

      if (next === undefined) {
        throw new Error('Fake transport stream was not configured.');
      }

      yield* next;
    },
  };
};

export const collect = async <T>(items: AsyncIterable<T>): Promise<readonly T[]> => {
  const collected: T[] = [];

  for await (const item of items) {
    collected.push(item);
  }

  return collected;
};
