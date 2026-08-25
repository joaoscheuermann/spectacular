# oauth

Generic OAuth 2 authorization-code + PKCE helpers for Doric's TypeScript agent
core. The package owns OAuth profiles, token exchange, refresh, credential
rendering, explicit local callback server creation, browser opening, and fetch
transport wiring.

OAuth behavior is intentionally outside provider packages. Host code should
compose this package with provider packages by passing a rendered credential's
`authorization` value into the provider configuration.

## Use

```ts
import {
  createCodexOAuth,
  createFetchTransport,
  createLocalCallbackServer,
  openBrowser,
  type OAuthTokenRecord,
  type OAuthTokenStore,
} from 'oauth';

let stored: OAuthTokenRecord | undefined;
const tokenStore: OAuthTokenStore = {
  load: async () => stored,
  save: async (record) => {
    stored = record;
  },
};

const callbackServer = await createLocalCallbackServer();
const codex = createCodexOAuth({
  transport: createFetchTransport(),
  tokenStore,
  redirectUri: callbackServer.redirectUri,
  browserOpener: openBrowser,
  callbackServer,
});

try {
  await codex.authorize();
  const credential = await codex.credential();
  const authorization = credential.authorization;
  // Pass authorization to the provider transport; never log it.
} finally {
  await callbackServer.close();
}
```

The in-memory token store above is only an example. A real host should inject
storage appropriate for OAuth credentials and must not log or commit tokens.
`credential()` refreshes expiring records when a refresh token is available
and returns a ready-to-use `authorization` header value.

Use `createOAuthClient` with an `OAuthProfile` for providers other than Codex.
`resolveCodexAuth` is the separate helper for resolving existing Codex CLI
environment or `auth.json` credentials without starting a browser flow.

## Development

```console
npx nx build oauth
npx nx test oauth
```
