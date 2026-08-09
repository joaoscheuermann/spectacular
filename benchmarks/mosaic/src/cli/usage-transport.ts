import type { HttpRequest, HttpResponse, HttpTransport } from 'llms';

export interface RetrievalUsage {
  readonly operation: 'embedding' | 'rerank';
  readonly model: string;
  readonly inputTokens: number;
  readonly searchUnits: number;
  readonly documents: number;
}

export interface UsageTransport {
  readonly transport: HttpTransport;
  readonly bind: (
    observer: ((usage: RetrievalUsage) => Promise<void>) | undefined,
  ) => void;
  readonly take: (
    operation: RetrievalUsage['operation'],
    model: string,
  ) => RetrievalUsage | undefined;
}

const record = (
  value: unknown,
): Readonly<Record<string, unknown>> | undefined =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Readonly<Record<string, unknown>>)
    : undefined;

const count = (value: unknown, label: string, required: boolean): number => {
  if (value === undefined && !required) return 0;
  if (!Number.isSafeInteger(value) || (value as number) < 0) {
    throw new TypeError(`provider returned invalid ${label} usage`);
  }
  return value as number;
};

const requestBody = (
  request: HttpRequest,
): Readonly<Record<string, unknown>> => {
  const body = record(JSON.parse(request.body ?? 'null'));
  if (body === undefined)
    throw new TypeError('provider request body is invalid');
  return body;
};

const responseUsage = (
  request: HttpRequest,
  response: HttpResponse,
): RetrievalUsage | undefined => {
  const operation = request.url.endsWith('/embeddings')
    ? 'embedding'
    : request.url.endsWith('/rerank')
      ? 'rerank'
      : undefined;
  if (operation === undefined) return undefined;
  let responseBody: Readonly<Record<string, unknown>> | undefined;
  try {
    responseBody = record(JSON.parse(response.body));
  } catch {
    return undefined;
  }
  const usage = record(responseBody?.['usage']);
  if (usage === undefined) return undefined;
  const body = requestBody(request);
  const model = body['model'];
  if (typeof model !== 'string' || model.trim().length === 0) {
    throw new TypeError('provider request model is invalid');
  }
  const documents = Array.isArray(body['documents'])
    ? body['documents'].length
    : 0;
  return {
    operation,
    model,
    inputTokens: count(
      usage[operation === 'embedding' ? 'prompt_tokens' : 'total_tokens'],
      `${operation} tokens`,
      response.status < 400,
    ),
    searchUnits: count(
      usage['search_units'],
      'rerank search units',
      operation === 'rerank' && response.status < 400,
    ),
    documents,
  };
};

/** Captures authenticated retrieval usage before the public provider reduces it. */
export const createUsageTransport = (source: HttpTransport): UsageTransport => {
  const pending: RetrievalUsage[] = [];
  let observer: ((usage: RetrievalUsage) => Promise<void>) | undefined;
  const transport: HttpTransport = {
    async request(request) {
      const response = await source.request(request);
      const usage = responseUsage(request, response);
      if (usage !== undefined) {
        pending.push(usage);
        await observer?.(usage);
      }
      return response;
    },
    stream: (request) => source.stream(request),
  };
  const take = (
    operation: RetrievalUsage['operation'],
    model: string,
  ): RetrievalUsage | undefined => {
    const index = pending.findIndex(
      (usage) => usage.operation === operation && usage.model === model,
    );
    if (index < 0) return undefined;
    return pending.splice(index, 1)[0];
  };
  return {
    transport,
    take,
    bind: (value) => {
      observer = value;
    },
  };
};
