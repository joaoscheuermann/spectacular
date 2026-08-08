# tools

Provider-neutral tool contracts for the TypeScript agent core.

```ts
import { z } from 'zod';
import { createToolStorage, defineTool } from 'tool';

const lookup = defineTool({
  name: 'lookup',
  description: 'Lookup indexed context.',
  input: z.object({ query: z.string() }).strict(),
  output: z.object({ result: z.string() }).strict(),
  async execute(sandbox, { query }) {
    return { result: `result for ${query}` };
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

`defineTool` creates an inspectable factory with Zod `input` and `output`
schemas and a neutral definition containing materialized `inputSchema` and
`outputSchema`. Calling the factory binds its execution to a sandbox. Inputs
are validated before the handler runs, and handler results are validated
before execution resolves. Provider adapters remain responsible for converting
only supported definition fields into their native wire shape.

`createToolStorage` preserves registration order, rejects duplicate names,
parses provider tool-call arguments, validates payloads before execution, and
throws `ToolErrorObject` for structured failures.

## Building

Run `nx build tool` to build the library.

## Testing

Run `nx test tool` to compile and run the package tests.
