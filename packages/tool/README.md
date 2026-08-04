# tools

Provider-neutral tool contracts for the TypeScript agent core.

```ts
import { z } from 'zod';
import { createToolStorage, defineTool } from 'tool';

const lookup = defineTool({
  name: 'lookup',
  description: 'Lookup indexed context.',
  schema: z.object({ query: z.string() }),
  async execute(sandbox, { query }) {
    return `result for ${query}`;
  },
});
const tools = createToolStorage([lookup(sandbox)]);

const turn = await provider.complete({
  model: 'gpt-5',
  messages,
  tools: tools.definitions(),
});

for (const call of tools.calls(turn)) {
  const result = await tools.execute(call);

  messages = [
    ...messages,
    {
      role: 'tool',
      toolCallId: call.id,
      content: JSON.stringify(result),
    },
  ];
}
```

`defineTool` creates an inspectable factory with a neutral JSON Schema
definition. Calling the factory binds its execution to a sandbox. Provider
adapters remain responsible for converting the definition into their native
wire shape.

`createToolStorage` preserves registration order, rejects duplicate names,
parses provider tool-call arguments, validates payloads before execution, and
throws `ToolErrorObject` for structured failures.

## Building

Run `nx build tools` to build the library.

## Testing

Run `nx test tools` to compile and run the package tests.
