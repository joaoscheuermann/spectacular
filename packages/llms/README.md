# llms

Provider-neutral model and streaming boundaries for Doric's TypeScript
agent core. The package keeps provider credentials, HTTP, browser opening,
time, and callback handling injected so tests can use fakes and host surfaces
can own sensitive behavior.

## OpenRouter with an API key

```ts
import { createFetchTransport, createOpenRouterProvider } from 'llms';

const provider = createOpenRouterProvider({
  transport: createFetchTransport(),
  apiKey: process.env.OPENROUTER_API_KEY ?? '',
});

const result = await provider.complete({
  model: 'openai/gpt-5',
  messages: [{ role: 'user', content: 'Summarize the plan.' }],
  tools: [
    {
      name: 'lookup',
      inputSchema: { type: 'object', properties: {} },
    },
  ],
});
```

## OpenAI with an API key

```ts
import { createFetchTransport, createOpenAiProvider } from 'llms';

const provider = createOpenAiProvider({
  transport: createFetchTransport(),
  apiKey: process.env.OPENAI_API_KEY ?? '',
});

for await (const event of provider.stream({
  model: 'gpt-5-fast',
  messages: [
    { role: 'system', content: 'Be concise.' },
    { role: 'user', content: 'Draft the next step.' },
  ],
  flags: { reasoning: { effort: 'low' } },
})) {
  // The host decides how to display structured stream events.
  console.log(event);
}
```

`*-fast` OpenAI model aliases are sent to the Responses API without the
suffix and with `service_tier: "priority"`.

## OpenAI OAuth setup

```ts
import {
  createFetchTransport,
  runOpenAiOAuthCallbackFlow,
  type OpenAiAuthStore,
} from 'llms';

const authStore: OpenAiAuthStore = {
  async load() {
    return undefined;
  },
  async save(record) {
    // Persist in host-owned secure storage.
    console.log(record.expiresAt);
  },
};

await runOpenAiOAuthCallbackFlow({
  transport: createFetchTransport(),
  authStore,
  clientId: 'client-id',
  redirectUri: 'http://127.0.0.1:3000/callback',
  browserOpener: (url) => open(url),
  callbackServer: localCallbackServer,
});
```

The OAuth helpers support PKCE, refresh-before-use, and provider retry after a
401 when an `authStore` is injected into `createOpenAiProvider`.

## Fake transport tests

```ts
import { createOpenRouterProvider, type HttpTransport } from 'llms';

const requests = [];
const transport: HttpTransport = {
  async request(request) {
    requests.push(request);
    return {
      status: 200,
      headers: {},
      body: JSON.stringify({ choices: [{ message: { content: 'ok' } }] }),
    };
  },
  async *stream() {
    yield 'data: {"choices":[{"delta":{"content":"ok"}}]}\n\n';
    yield 'data: [DONE]\n\n';
  },
};

const provider = createOpenRouterProvider({ transport, apiKey: 'test-key' });
```

## Building

Run `nx build llms` to build the library.

## Testing

Run `nx test llms` to compile and run the package tests.
