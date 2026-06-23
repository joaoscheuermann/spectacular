import express from 'express';
import cors from 'cors';
import pino from 'pino';

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
const logLevel = process.env.LOG_LEVEL?.trim() || 'info';
const logger = pino({ level: logLevel });
const httpLogger = logger.child({ component: 'http' });

const sessions = createSessionStore<DoricSessionContext>();

const card = createAgentCard(`${host}:${port}`);

const tasks = new InMemoryTaskStore();

const executor = createExecutor({
  sessions,
  createDockerClient,
  createSandbox,
  logger: logger.child({ component: 'executor' }),
});

const requestHandler = new DefaultRequestHandler(card, tasks, executor);

app.use((request, response, next) => {
  const startedAt = process.hrtime.bigint();

  response.on('finish', () => {
    const durationMs = Number(process.hrtime.bigint() - startedAt) / 1_000_000;

    httpLogger.info(
      {
        method: request.method,
        path: request.path,
        statusCode: response.statusCode,
        durationMs,
      },
      'HTTP request completed',
    );
  });

  next();
});
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
  logger.info(
    {
      host,
      port,
      discoveryPath: '/.well-known/a2a-agent-card',
      rpcPath: '/rpc',
    },
    'Doric A2A server listening',
  );
});
