import { randomUUID } from 'node:crypto';

import type { Message } from '@a2a-js/sdk';
import {
  A2AError,
  type AgentExecutor,
  type RequestContext,
} from '@a2a-js/sdk/server';
import { ConfigParseError, parseInitialMessageConfig } from 'config';

import { HELLO_WORLD_TEXT } from './card.js';

export const executor: AgentExecutor = {
  execute: (requestContext, eventBus) => {
    validateConfig(requestContext);

    eventBus.publish(createHelloWorldMessage(requestContext.contextId));
    eventBus.finished();

    return Promise.resolve();
  },
  cancelTask: () => Promise.resolve(),
};

const validateConfig = (requestContext: RequestContext): void => {
  try {
    parseInitialMessageConfig(requestContext.userMessage);
  } catch (error) {
    if (error instanceof ConfigParseError) {
      throw A2AError.invalidParams(error.message, {
        code: error.code,
        path: error.path,
      });
    }

    throw error;
  }
};

const createHelloWorldMessage = (contextId: string): Message => ({
  kind: 'message',
  messageId: randomUUID(),
  role: 'agent',
  parts: [{ kind: 'text', text: HELLO_WORLD_TEXT }],
  contextId,
});
