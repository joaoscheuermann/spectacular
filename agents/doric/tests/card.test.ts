import assert from 'node:assert/strict';
import test from 'node:test';

import { createAgentCard } from '../src/lib/card.js';

test('creates the Doric agent card for the configured host', () => {
  assert.deepEqual(createAgentCard('127.0.0.1:4123'), {
    name: 'doric',
    description: 'Doric A2A Coding Agent.',
    protocolVersion: '0.3.0',
    url: 'http://127.0.0.1:4123/rpc',
    preferredTransport: 'JSONRPC',
    additionalInterfaces: [
      { url: 'http://127.0.0.1:4123/rpc', transport: 'JSONRPC' },
    ],
    provider: {
      organization: 'Doric',
      url: 'https://github.com/joaoscheuermann/doric',
    },
    version: '0.0.1',
    capabilities: {
      streaming: true,
      pushNotifications: true,
    },
    defaultInputModes: ['text'],
    defaultOutputModes: ['text'],
    skills: [
      {
        id: 'feature',
        name: 'Feature Development Especialist',
        description:
          'Develops a Feature for you, using PRD, TDD and task decompoisition',
        tags: ['feature', 'coding', 'tdd', 'prd'],
        inputModes: ['text'],
        outputModes: ['text'],
      },
    ],
    supportsAuthenticatedExtendedCard: false,
  });
});
