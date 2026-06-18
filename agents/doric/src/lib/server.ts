import { createServer as createHttpServer, type Server } from 'node:http';

import { AGENT_CARD_PATH } from '@a2a-js/sdk';
import { DefaultRequestHandler, InMemoryTaskStore } from '@a2a-js/sdk/server';
import { jsonRpcHandler, UserBuilder } from '@a2a-js/sdk/server/express';
import express, { type Request } from 'express';

import { createAgentCard, RPC_PATH } from './card.js';
import { executor } from './executor.js';

/** Creates a Node HTTP server for Doric's minimal A2A protocol surface. */
export const createDoricServer = (): Server => {
  const requestHandler = createRequestHandler();
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

const createRequestHandler = (): DefaultRequestHandler =>
  new DefaultRequestHandler(
    createAgentCard(`http://localhost${RPC_PATH}`),
    new InMemoryTaskStore(),
    executor,
  );

const createRpcUrl = (request: Request): string => {
  const protocol = request.protocol;
  const host = request.get('host') ?? 'localhost';

  return new URL(RPC_PATH, `${protocol}://${host}`).toString();
};
