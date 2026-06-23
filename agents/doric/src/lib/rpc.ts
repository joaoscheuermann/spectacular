import type { ErrorRequestHandler, Request, RequestHandler } from 'express';
import express from 'express';

import {
  Extensions,
  HTTP_EXTENSION_HEADER,
  type Message,
  type JSONRPCError,
} from '@a2a-js/sdk';
import {
  A2AError,
  JsonRpcTransportHandler,
  ServerCallContext,
  type A2ARequestHandler,
} from '@a2a-js/sdk/server';
import type { User } from '@a2a-js/sdk/server';

import type { SessionStore } from 'session';

import type { DoricSessionContext } from './executor.js';
import type { RuntimeStore } from './runtime/store.js';

type UserBuilder = (request: Request) => Promise<User>;

type JsonRpcId = string | number | null;

type JsonRpcRequest = {
  readonly jsonrpc: '2.0';
  readonly id?: JsonRpcId;
  readonly method: string;
  readonly params?: unknown;
};

type JsonRpcResponseMessage = {
  readonly jsonrpc: '2.0';
  readonly id: JsonRpcId;
  readonly result?: unknown;
  readonly error?: JSONRPCError;
};

type DoricRpcHandlerOptions = {
  readonly requestHandler: A2ARequestHandler;
  readonly runtime: RuntimeStore;
  readonly sessions: SessionStore<DoricSessionContext>;
  readonly userBuilder: UserBuilder;
};

const SSE_HEADERS = {
  'Content-Type': 'text/event-stream',
  'Cache-Control': 'no-cache',
  Connection: 'keep-alive',
  'X-Accel-Buffering': 'no',
};

/** Creates Doric's JSON-RPC route with SDK delegation and custom session methods. */
export const createDoricJsonRpcHandler = (
  options: DoricRpcHandlerOptions,
): RequestHandler => {
  const router = express.Router();
  const handler = createDoricRpcHandler(options);

  router.use(express.json(), jsonErrorHandler);
  router.post('/', async (request, response) => {
    try {
      const context = await serverCallContext(request, options.userBuilder);
      const rpcResponseOrStream = await handler.handle(request.body, context);

      if (context.activatedExtensions) {
        response.setHeader(
          HTTP_EXTENSION_HEADER,
          Array.from(context.activatedExtensions),
        );
      }

      if (isAsyncIterable(rpcResponseOrStream)) {
        for (const [key, value] of Object.entries(SSE_HEADERS)) {
          response.setHeader(key, value);
        }

        response.flushHeaders();

        try {
          for await (const event of rpcResponseOrStream) {
            response.write(formatSseEvent(event));
          }
        } catch (error) {
          const rpcError = toA2AError(error, 'Streaming error.');
          response.write(
            formatSseErrorEvent(errorResponse(request.body?.id, rpcError)),
          );
        } finally {
          if (!response.writableEnded) {
            response.end();
          }
        }

        return;
      }

      response.status(200).json(rpcResponseOrStream);
    } catch (error) {
      const rpcError = toA2AError(error, 'General processing error.');

      if (!response.headersSent) {
        response.status(500).json(errorResponse(request.body?.id, rpcError));
      } else if (!response.writableEnded) {
        response.end();
      }
    }
  });

  return router;
};

export const createDoricRpcHandler = ({
  requestHandler,
  runtime,
  sessions,
}: DoricRpcHandlerOptions) => {
  const standard = new JsonRpcTransportHandler(requestHandler);

  return {
    async handle(
      requestBody: unknown,
      context?: ServerCallContext,
    ): Promise<
      JsonRpcResponseMessage | AsyncGenerator<JsonRpcResponseMessage, void>
    > {
      const request = parseRequest(requestBody);

      if ('error' in request) {
        return request.error;
      }

      if (!request.value.method.startsWith('doric/')) {
        observeStandardMessage(runtime, request.value);

        return (await standard.handle(
          requestBody,
          context,
        )) as JsonRpcResponseMessage;
      }

      return handleDoricMethod({
        request: request.value,
        context,
        requestHandler,
        runtime,
        sessions,
      });
    },
  };
};

const handleDoricMethod = async ({
  request,
  context,
  requestHandler,
  runtime,
  sessions,
}: {
  readonly request: JsonRpcRequest;
  readonly context?: ServerCallContext;
  readonly requestHandler: A2ARequestHandler;
  readonly runtime: RuntimeStore;
  readonly sessions: SessionStore<DoricSessionContext>;
}): Promise<
  JsonRpcResponseMessage | AsyncGenerator<JsonRpcResponseMessage, void>
> => {
  const requestId = request.id ?? null;

  try {
    switch (request.method) {
      case 'doric/sessions/list':
        return successResponse(requestId, sessionListResponse(runtime));

      case 'doric/sessions/connect':
        return connectResponse(requestId, runtime, contextIdParam(request));

      case 'doric/sessions/kill':
        return successResponse(
          requestId,
          await killSession({
            contextId: contextIdParam(request),
            context,
            requestHandler,
            runtime,
            sessions,
          }),
        );

      default:
        throw A2AError.methodNotFound(request.method);
    }
  } catch (error) {
    return errorResponse(requestId, toA2AError(error));
  }
};

const connectResponse = (
  requestId: JsonRpcId,
  runtime: RuntimeStore,
  contextId: string,
): AsyncGenerator<JsonRpcResponseMessage, void> => {
  if (!runtime.has(contextId)) {
    throw A2AError.taskNotFound(contextId);
  }

  return (async function* stream() {
    for await (const event of runtime.connect(contextId)) {
      yield successResponse(requestId, event);
    }
  })();
};

const sessionListResponse = (runtime: RuntimeStore) => ({
  sessions: runtime
    .list()
    .map(({ contextId, state, prompt }) => ({ contextId, state, prompt })),
});

const killSession = async ({
  contextId,
  context,
  requestHandler,
  runtime,
  sessions,
}: {
  readonly contextId: string;
  readonly context?: ServerCallContext;
  readonly requestHandler: A2ARequestHandler;
  readonly runtime: RuntimeStore;
  readonly sessions: SessionStore<DoricSessionContext>;
}): Promise<{ readonly contextId: string; readonly killed: true }> => {
  const sessionPromise = sessions.load(contextId);
  const latestTaskId = runtime.latestTaskId(contextId);

  if (!runtime.has(contextId) && sessionPromise === undefined) {
    throw A2AError.taskNotFound(contextId);
  }

  if (latestTaskId !== undefined) {
    await requestHandler.cancelTask({ id: latestTaskId }, context).catch(
      () => undefined,
    );
  }

  sessions.delete(contextId);
  runtime.delete(contextId);

  const session = await sessionPromise?.catch(() => undefined);
  await session?.sandbox.dispose().catch(() => undefined);

  return { contextId, killed: true };
};

const observeStandardMessage = (
  runtime: RuntimeStore,
  request: JsonRpcRequest,
): void => {
  if (request.method !== 'message/send' && request.method !== 'message/stream') {
    return;
  }

  const params = request.params;

  if (!isRecord(params) || !isRecord(params['message'])) {
    return;
  }

  runtime.observeMessage(params['message'] as unknown as Message);
};

const parseRequest = (
  requestBody: unknown,
): { readonly value: JsonRpcRequest } | { readonly error: JsonRpcResponseMessage } => {
  let parsed: unknown;

  try {
    parsed = typeof requestBody === 'string' ? JSON.parse(requestBody) : requestBody;
  } catch (error) {
    return { error: errorResponse(null, toA2AError(error, 'Failed to parse JSON request.')) };
  }

  if (!isRecord(parsed) || parsed['jsonrpc'] !== '2.0') {
    return {
      error: errorResponse(null, A2AError.invalidRequest('Invalid JSON-RPC Request.')),
    };
  }

  const id = parsed['id'];
  const method = parsed['method'];

  if (
    (id !== undefined && !isJsonRpcId(id)) ||
    typeof method !== 'string' ||
    method === ''
  ) {
    return {
      error: errorResponse(
        isJsonRpcId(id) ? id : null,
        A2AError.invalidRequest('Invalid JSON-RPC Request.'),
      ),
    };
  }

  return {
    value: {
      jsonrpc: '2.0',
      ...(id === undefined ? {} : { id }),
      method,
      params: parsed['params'],
    },
  };
};

const contextIdParam = (request: JsonRpcRequest): string => {
  if (!isRecord(request.params) || typeof request.params['contextId'] !== 'string') {
    throw A2AError.invalidParams('contextId is required.', {
      code: 'missing_context_id',
      path: 'params.contextId',
    });
  }

  return request.params['contextId'];
};

const serverCallContext = async (
  request: Request,
  userBuilder: UserBuilder,
): Promise<ServerCallContext> =>
  new ServerCallContext(
    Extensions.parseServiceParameter(request.header(HTTP_EXTENSION_HEADER)),
    await userBuilder(request),
  );

const successResponse = (
  id: JsonRpcId,
  result: unknown,
): JsonRpcResponseMessage => ({
  jsonrpc: '2.0',
  id,
  result,
});

const errorResponse = (
  id: JsonRpcId | undefined,
  error: A2AError,
): JsonRpcResponseMessage => ({
  jsonrpc: '2.0',
  id: id ?? null,
  error: error.toJSONRPCError(),
});

const toA2AError = (
  error: unknown,
  fallback = 'An unexpected error occurred.',
): A2AError => {
  if (error instanceof A2AError) {
    return error;
  }

  return A2AError.internalError(
    error instanceof Error ? error.message : fallback,
  );
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const isJsonRpcId = (value: unknown): value is JsonRpcId =>
  value === null ||
  typeof value === 'string' ||
  (typeof value === 'number' && Number.isInteger(value));

const isAsyncIterable = (
  value: unknown,
): value is AsyncIterable<JsonRpcResponseMessage> =>
  typeof (value as AsyncIterable<unknown> | undefined)?.[
    Symbol.asyncIterator
  ] === 'function';

const formatSseEvent = (event: JsonRpcResponseMessage): string =>
  `data: ${JSON.stringify(event)}\n\n`;

const formatSseErrorEvent = (event: JsonRpcResponseMessage): string =>
  `event: error\ndata: ${JSON.stringify(event)}\n\n`;

const jsonErrorHandler: ErrorRequestHandler = (error, _request, response, next) => {
  if (error instanceof SyntaxError && 'body' in error) {
    response
      .status(400)
      .json(errorResponse(null, A2AError.parseError('Invalid JSON payload.')));
    return;
  }

  next(error);
};
