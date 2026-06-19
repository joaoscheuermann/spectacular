import express from 'express';
import cors from 'cors';

import { UserBuilder } from '@a2a-js/sdk/server/grpc';
import { agentCardHandler, jsonRpcHandler } from '@a2a-js/sdk/server/express';
import { DefaultRequestHandler, InMemoryTaskStore } from '@a2a-js/sdk/server';

import { createSessionStore } from 'session';
import { createSandbox } from 'sandbox';
import { createDockerClient } from 'docker';

import { DEFAULT_HOST, DEFAULT_PORT } from './lib/constants/server.js';

import { createAgentCard } from './lib/card.js';
import { createExecutor, DoricSessionContext } from './lib/executor.js';

const app = express();

const host = String(process.env.HOST || DEFAULT_HOST);
const port = Number(process.env.PORT || DEFAULT_PORT);

const sessions = createSessionStore<DoricSessionContext>();

const card = createAgentCard(`${host}:${port}`);

const tasks = new InMemoryTaskStore();

const executor = createExecutor({
  sessions,
  createDockerClient,
  createSandbox,
});

const requestHandler = new DefaultRequestHandler(card, tasks, executor);

app.use(cors());
app.use(express.json());

// Sends the agent card for the other Agent
app.use(
  '/.well-known/a2a-agent-card',
  agentCardHandler({ agentCardProvider: async () => card }),
);

// Standard JSON-RPC Endpoint (Recommended by the protocol)
app.use(
  '/rpc',
  jsonRpcHandler({ requestHandler, userBuilder: UserBuilder.noAuthentication }),
);

app.listen(port, host, () => {
  console.log(`A2A Express Server listening at http://localhost:${port}`);
  console.log(
    `Discovery endpoint available at http://localhost:${port}/.well-known/a2a-agent-card`,
  );
});
