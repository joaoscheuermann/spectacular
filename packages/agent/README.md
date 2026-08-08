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
import pino from 'pino';
import { createToolStorage, defineTool } from 'tool';
import { z } from 'zod';

const logger = pino();

const provider = createOpenAiProvider({
  transport: createFetchTransport(),
  apiKey: process.env.CODEX_API_KEY ?? '',
  logger,
});

const lookup = defineTool({
  name: 'lookup',
  description: 'Lookup indexed project context.',
  input: z.object({ query: z.string() }).strict(),
  output: z.object({ result: z.string() }).strict(),
  async execute(sandbox, { query }) {
    return { result: `context for ${query}` };
  },
});
const tools = createToolStorage([lookup(sandbox)]);

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

Each `complete` or `stream` run may set `maxTurns` to a positive safe integer.
One turn is one provider invocation, including a direct or terminal response,
a response containing any number of tool calls, or a structured-output repair
attempt. Omitting the option preserves an unbounded loop. Invalid values throw
`TypeError` before the input is stored or the provider is called.

When the limit is reached, tools requested by the final permitted turn still
execute and their results are stored. The run then throws an `AgentErrorObject`
with code `turn_limit_exceeded` before another provider invocation. Streaming
preserves events already emitted but does not emit `agent.finished` for an
exhausted run.

Every run that requests structured output adds one strict terminal tool derived
from the schema, even when the caller's tool storage is empty. The schema is
never sent as a provider-native structured-output field by the agent. Ordinary
executable tool calls continue the loop; the terminal call is validated
locally, converted to the final structured response, and is never executed.

An invalid terminal submission is discarded without storage or tool execution.
The agent may request three corrected submissions after the initial failure,
using a transient system correction that names the terminal tool and reports
bounded schema issues without copying the rejected arguments. Missing,
malformed, schema-invalid, duplicate, and mixed terminal calls share this fixed
budget. Ordinary tool turns neither consume nor reset it. The fourth invalid
submission throws the latest `invalid_structured_output` error.

`complete` and `stream` use the same repair behavior. Streaming preserves
provider deltas already emitted for a rejected response but suppresses its
`response.finished` event and adds no repair-specific public event.

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
messages, and the provider loop continues until a response has no tool calls or
submits a validated terminal structured output.

## Building

Run `nx build agent` to build the library.

## Testing

Run `nx test agent` to compile and run the node:test coverage.
