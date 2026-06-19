import { createServer as createHttpServer, type Server } from 'node:http';

import { AGENT_CARD_PATH } from '@a2a-js/sdk';
import {
  DefaultRequestHandler,
  InMemoryTaskStore,
  type AgentExecutor,
} from '@a2a-js/sdk/server';
import { jsonRpcHandler, UserBuilder } from '@a2a-js/sdk/server/express';
import express, { type Request } from 'express';
import { createSessionStore } from 'session';

import { createAgentCard } from './card.js';
import { RPC_PATH } from './constants/agent.js';
import { createExecutor, type DoricSessionContext } from './executor.js';

export type DoricServerOptions = {
  readonly executor?: AgentExecutor;
};

/** Creates a Node HTTP server for Doric's minimal A2A protocol surface. */
export const createServer = (options: DoricServerOptions = {}): Server => {
  const agentExecutor =
    options.executor ??
    createExecutor({
      sessions: createSessionStore<DoricSessionContext>(),
    });
  const requestHandler = createRequestHandler(agentExecutor);
  const app = express();

  app.get(`/${AGENT_CARD_PATH}`, (request, response) => {
    response.json(createAgentCard(createRpcUrl(request)));
  });
  app.use(
    RPC_PATH,
    jsonRpcHandler({
      requestHandler,
      userBuilder: UserBuilder.noAuthentication,
    }),
  );

  return createHttpServer(app);
};

const createRequestHandler = (
  agentExecutor: AgentExecutor,
): DefaultRequestHandler =>
  new DefaultRequestHandler(
    createAgentCard(`http://localhost${RPC_PATH}`),
    new InMemoryTaskStore(),
    agentExecutor,
  );

const createRpcUrl = (request: Request): string => {
  const protocol = request.protocol;
  const host = request.get('host') ?? 'localhost';

  return new URL(RPC_PATH, `${protocol}://${host}`).toString();
};
