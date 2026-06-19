import type { AgentCard } from '@a2a-js/sdk';

export const createAgentCard = (host: string): AgentCard => ({
  name: 'doric',
  description: 'Doric A2A Coding Agent.',
  protocolVersion: '0.3.0',
  url: `http://${host}/rpc`,
  preferredTransport: 'JSONRPC',
  additionalInterfaces: [{ url: `http://${host}/rpc`, transport: 'JSONRPC' }],
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
