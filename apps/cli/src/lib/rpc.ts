import { TextDecoder } from 'node:util';

import type { AgentCard } from '@a2a-js/sdk';

export type JsonRpcClient = {
  call<Result>(method: string, params: unknown): Promise<Result>;
  stream<Result>(method: string, params: unknown): AsyncGenerator<Result, void>;
};

type JsonRpcResponse<Result> =
  | {
      readonly jsonrpc: '2.0';
      readonly id: number;
      readonly result: Result;
    }
  | {
      readonly jsonrpc: '2.0';
      readonly id: number;
      readonly error: {
        readonly code: number;
        readonly message: string;
        readonly data?: unknown;
      };
    };

export class JsonRpcCliError extends Error {
  readonly code: number;
  readonly data: unknown;

  constructor(error: { readonly code: number; readonly message: string; readonly data?: unknown }) {
    super(error.message);
    this.name = 'JsonRpcCliError';
    this.code = error.code;
    this.data = error.data;
  }
}

export const fetchAgentCard = async (
  cardUrl: string,
  fetchImpl: typeof fetch = fetch,
): Promise<AgentCard> => {
  const response = await fetchImpl(cardUrl, {
    headers: { Accept: 'application/json' },
  });

  if (!response.ok) {
    throw new Error(
      `Failed to fetch Agent Card from ${cardUrl}: ${response.status} ${response.statusText}`,
    );
  }

  const card = (await response.json()) as AgentCard;

  if (typeof card.url !== 'string' || card.url === '') {
    throw new Error('Agent Card does not include a JSON-RPC URL.');
  }

  return card;
};

export const createJsonRpcClient = (
  endpoint: string,
  fetchImpl: typeof fetch = fetch,
): JsonRpcClient => {
  let nextId = 1;

  const request = (method: string, params: unknown) => ({
    jsonrpc: '2.0',
    id: nextId++,
    method,
    params,
  });

  return {
    async call<Result>(method: string, params: unknown): Promise<Result> {
      const body = request(method, params);
      const response = await fetchRpc(fetchImpl, endpoint, body, 'application/json');
      const payload = (await response.json()) as JsonRpcResponse<Result>;

      return resultFor(payload, body.id);
    },

    async *stream<Result>(
      method: string,
      params: unknown,
    ): AsyncGenerator<Result, void> {
      const body = request(method, params);
      const response = await fetchRpc(fetchImpl, endpoint, body, 'text/event-stream');
      const contentType = response.headers.get('content-type');

      if (!contentType?.startsWith('text/event-stream')) {
        const payload = (await response.json().catch(() => undefined)) as
          | JsonRpcResponse<unknown>
          | undefined;

        if (payload !== undefined) {
          resultFor(payload, body.id);
        }

        throw new Error(
          `Invalid response Content-Type for SSE stream: ${contentType ?? '(missing)'}.`,
        );
      }

      for await (const event of parseSse(response)) {
        yield resultFor(JSON.parse(event.data) as JsonRpcResponse<Result>, body.id);
      }
    },
  };
};

const fetchRpc = async (
  fetchImpl: typeof fetch,
  endpoint: string,
  body: { readonly id: number; readonly method: string; readonly params: unknown },
  accept: string,
): Promise<Response> => {
  const response = await fetchImpl(endpoint, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: accept,
    },
    body: JSON.stringify(body),
  });

  if (!response.ok) {
    throw new Error(
      `HTTP error for ${body.method}: ${response.status} ${response.statusText}. ${await response.text()}`,
    );
  }

  return response;
};

const resultFor = <Result>(
  response: JsonRpcResponse<Result>,
  expectedId: number,
): Result => {
  if (response.id !== expectedId) {
    throw new Error(
      `JSON-RPC response ID mismatch. Expected ${expectedId}, got ${response.id}.`,
    );
  }

  if ('error' in response) {
    throw new JsonRpcCliError(response.error);
  }

  return response.result;
};

type SseEvent = {
  readonly type: string;
  readonly data: string;
};

const parseSse = async function* (response: Response): AsyncGenerator<SseEvent, void> {
  if (response.body === null) {
    throw new Error('SSE response body is empty.');
  }

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let eventType = 'message';
  let eventData = '';

  try {
    while (true) {
      const { done, value } = await reader.read();

      if (done) {
        break;
      }

      buffer += decoder.decode(value, { stream: true });
      let lineEnd = buffer.indexOf('\n');

      while (lineEnd >= 0) {
        const line = buffer.slice(0, lineEnd).trim();
        buffer = buffer.slice(lineEnd + 1);

        if (line === '') {
          if (eventData !== '') {
            yield { type: eventType, data: eventData };
            eventType = 'message';
            eventData = '';
          }
        } else if (line.startsWith('event:')) {
          eventType = line.slice('event:'.length).trim();
        } else if (line.startsWith('data:')) {
          eventData = line.slice('data:'.length).trim();
        }

        lineEnd = buffer.indexOf('\n');
      }
    }

    if (eventData !== '') {
      yield { type: eventType, data: eventData };
    }
  } finally {
    reader.releaseLock();
  }
};
