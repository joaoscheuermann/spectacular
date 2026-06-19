import assert from 'node:assert/strict';
import test from 'node:test';

import { HELLO_WORLD_TEXT } from '../src/lib/constants/agent.js';
import { createHelloWorldMessage } from '../src/lib/messages/hello-world.js';

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

test('creates a hello-world agent message for the provided context', () => {
  const message = createHelloWorldMessage('context-1');

  assert.equal(message.kind, 'message');
  assert.match(message.messageId, UUID_PATTERN);
  assert.equal(message.role, 'agent');
  assert.equal(message.contextId, 'context-1');
  assert.deepEqual(message.parts, [{ kind: 'text', text: HELLO_WORLD_TEXT }]);
});
