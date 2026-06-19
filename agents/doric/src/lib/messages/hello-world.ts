import { randomUUID } from 'node:crypto';

import type { Message } from '@a2a-js/sdk';

import { HELLO_WORLD_TEXT } from '../constants/agent.js';

export const createHelloWorldMessage = (contextId: string): Message => ({
  kind: 'message',
  messageId: randomUUID(),
  role: 'agent',
  parts: [{ kind: 'text', text: HELLO_WORLD_TEXT }],
  contextId,
});
