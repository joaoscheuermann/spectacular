import { createAgent } from 'agent';
import {
  createFetchTransport,
  createLmStudioOpenAiProvider,
  // createOpenRouterProvider,
  ReasoningEffort,
} from 'llms';
import { createMessageStorage } from 'messages';
import { createToolStorage } from 'tools';
import { BASE_URL } from './constants.js';

export const agent = (
  model: string,
  system: string,
  effort: ReasoningEffort,
) => {
  return createAgent({
    provider: createLmStudioOpenAiProvider({
      // provider: createOpenRouterProvider({
      // debugLogger: console as any,
      transport: createFetchTransport(),
      baseUrl: BASE_URL,
    }),
    tools: createToolStorage([]),
    messages: createMessageStorage(),
    system,
    effort,
    model,
    temperature: 0,
  });
};
