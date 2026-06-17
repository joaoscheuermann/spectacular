# oauth

Generic OAuth 2 authorization-code + PKCE helpers for Doric's TypeScript
agent core. The package owns OAuth profiles, token exchange, refresh,
credential rendering, explicit local callback server creation, explicit
browser opening, and fetch transport wiring.

OAuth behavior is intentionally outside provider packages. Host code should
compose this package with provider packages by passing a rendered credential's
`authorization` value into the provider configuration.

```ts
import {
  createFetchTransport,
  createLocalCallbackServer,
  createOpenAiOAuth,
  openBrowser,
} from 'oauth';

const callbackServer = await createLocalCallbackServer();
const openai = createOpenAiOAuth({
  transport: createFetchTransport(),
  tokenStore,
  clientId: 'client-id',
  redirectUri: callbackServer.redirectUri,
  browserOpener: openBrowser,
  callbackServer,
});

await openai.authorize();
const credential = await openai.oauth();
```

## Building

Run `nx build oauth` to build the library.

## Testing

Run `nx test oauth` to compile and run the package tests.
