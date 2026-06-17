# agent

Embeddable TypeScript agent core that composes an injected LLM provider,
message storage, and tool storage.

The package owns agent loop control, request assembly, tool execution order,
stream events, and agent-specific lifecycle errors. Provider and tool package
errors pass through unchanged.

## Basic Usage

```ts
import { createAgent } from 'agent';
import { createFetchTransport, createOpenAiProvider } from 'llms';
import { createMessageStorage } from 'messages';
import { createTool, createToolStorage } from 'tools';
import { z } from 'zod';

const provider = createOpenAiProvider({
  transport: createFetchTransport(),
  apiKey: process.env.OPENAI_API_KEY ?? '',
});

const tools = createToolStorage([
  createTool({
    name: 'lookup',
    description: 'Lookup indexed project context.',
    schema: z.object({ query: z.string() }),
    async execute({ query }) {
      return { result: `context for ${query}` };
    },
  }),
]);

const messages = createMessageStorage();

const agent = createAgent({
  provider,
  tools,
  messages,
  system: 'You are a concise project assistant.',
  model: 'gpt-5',
  flags: { reasoning: { effort: 'low' }, includeUsage: true },
});

const response = await agent.complete('What should I work on next?');

console.log(response.text);
console.log(messages.list());
```

`createAgent` does not create defaults. The caller owns provider credentials,
tool registration, message storage, model selection, and per-call cancellation.
The system prompt is included in provider requests but is not persisted into
external message storage.

## Streaming Usage

```ts
for await (const event of agent.stream('Find the current project status.')) {
  switch (event.type) {
    case 'text.delta':
      process.stdout.write(event.delta);
      break;

    case 'tool.started':
      console.log(`Calling ${event.call.name}`);
      break;

    case 'tool.finished':
      console.log(`Tool result: ${event.content}`);
      break;

    case 'agent.finished':
      console.log(event.response.finishReason);
      break;
  }
}
```

Streaming passes through provider events and adds agent lifecycle and tool
events. Tool calls are executed sequentially in provider order, stored as tool
messages, and the provider loop continues until a response has no tool calls.

## Building

Run `nx build agent` to build the library.

## Testing

Run `nx test agent` to compile and run the node:test coverage.
