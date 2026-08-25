# messages

In-memory provider-ready conversation history for the TypeScript agent core.
The package stores replayable `ProviderMessage` entries and normalizes completed
provider turns into assistant messages.

## Basic Usage

```ts
import { createMessageStorage } from 'messages';
import type { ProviderMessage, ProviderRequest } from 'llms';

const initial: readonly ProviderMessage[] = [
  { role: 'system', content: 'You are concise.' },
  { role: 'user', content: 'Summarize the current task.' },
];

const messages = createMessageStorage(initial);

messages.push({
  role: 'assistant',
  content: 'We need to update the README examples.',
});

const request: ProviderRequest = {
  model: 'example-model',
  messages: messages.list(),
};
```

`createMessageStorage` copies the initial array, and `list()` returns a new
array snapshot. Mutating the caller-owned initial array or a previous list
snapshot does not change stored history.

## Appending User And Tool Messages

```ts
import { createMessageStorage } from 'messages';

const messages = createMessageStorage();

messages.push({
  role: 'user',
  content: 'Look up the project status.',
});

messages.push({
  role: 'tool',
  toolCallId: 'call_1',
  content: '{"status":"ready"}',
});

const count = messages.push({
  role: 'assistant',
  content: 'The project is ready.',
});

console.log(count); // 3
console.log(messages.list());
```

`push()` returns the new message count after storing the entry.

## Normalizing Provider Results

Provider completion results can be pushed directly. The storage converts them
into replayable assistant messages and keeps tool calls when present.

```ts
import { createMessageStorage } from 'messages';
import type { ProviderFinished } from 'llms';

const messages = createMessageStorage([
  { role: 'user', content: 'Call the lookup tool.' },
]);

const finished: ProviderFinished = {
  text: 'I will call the lookup tool.',
  finishReason: 'tool_calls',
  toolCalls: [
    {
      id: 'call_1',
      name: 'lookup',
      arguments: '{"query":"doric"}',
      index: 0,
    },
  ],
};

messages.push(finished);

console.log(messages.list());
// [
//   { role: 'user', content: 'Call the lookup tool.' },
//   {
//     role: 'assistant',
//     content: 'I will call the lookup tool.',
//     toolCalls: finished.toolCalls,
//   },
// ]
```

Usage metadata and reasoning metadata from `ProviderFinished` are intentionally
not stored in replayable messages. If a provider returns empty text with a
refusal, the refusal text becomes assistant content.

## Building

Run `nx build messages` to build the library.

## Testing

Run `nx test messages` to compile and run the package tests.
