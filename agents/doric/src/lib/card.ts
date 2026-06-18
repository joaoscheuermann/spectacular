import type { AgentCard } from '@a2a-js/sdk';

export const HELLO_WORLD_TEXT = 'Hello world from Doric.';
export const RPC_PATH = '/rpc';

export const createAgentCard = (url: string): AgentCard => ({
  name: 'doric',
  description: 'Doric A2A Coding Agent.',
  protocolVersion: '0.3.0',
  url,
  preferredTransport: 'JSONRPC',
  additionalInterfaces: [{ url, transport: 'JSONRPC' }],
  provider: {
    organization: 'Doric',
    url: 'https://github.com/joaoscheuermann/spectacular',
  },
  version: '0.0.1',
  capabilities: {
    pushNotifications: false,
    streaming: false,
  },
  defaultInputModes: ['text'],
  defaultOutputModes: ['text'],
  skills: [
    {
      id: 'hello-world',
      name: 'Hello world',
      description: 'Returns a hello world message.',
      tags: ['hello-world', 'text'],
      inputModes: ['text'],
      outputModes: ['text'],
    },
  ],
  supportsAuthenticatedExtendedCard: false,
});
