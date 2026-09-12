# Doric

Doric is a TypeScript coding agent that keeps a conversation and its working
environment alive across multiple prompts.

## How Direct works

1. A client creates a session through the REST API.
2. Doric reserves one isolated Docker or Firecracker sandbox for that session.
3. Prompts enter a FIFO queue and run one at a time in the same sandbox.
4. Each prompt gets a fresh Agent instance with the configured model, persisted
   conversation history, built-in skills, and sandbox-bound tools.
5. PostgreSQL stores configuration, sessions, messages, and the ordered event
   stream. Socket.IO replays stored events before delivering live updates.
6. The sandbox remains reserved until the session is terminated. Sessions do
   not expire automatically.

## Start Direct locally

With PostgreSQL and Docker running:

```console
npm ci
# Set DORIC_DATABASE_URL and OPENROUTER_API_KEY in .env.
npx nx run doric:migrate
npx nx serve doric
```

Doric listens on `0.0.0.0:3000` by default. The API is unauthenticated, so keep
it on a trusted network.

See [`agents/doric/README.md`](agents/doric/README.md) for the API, event, and
persistence contracts.

## Formatting and linting

Use Node.js 20.19+, 22.13+, or 24+ for the development tools.

```console
npm run format:check  # Check formatting across the workspace
npm run format       # Apply Prettier formatting
npm run lint         # Check JavaScript and TypeScript, including scripts/*.mjs
npm run lint:fix     # Apply available ESLint fixes; review the diff afterward
npx nx lint agent   # Lint one Nx project
```

Prettier uses its defaults with single quotes. ESLint uses the recommended
JavaScript and TypeScript rules, with type-aware checks for package source,
tools, and tests. Formatting rules are delegated to Prettier. Braces are
required, nested ternaries are rejected, and complexity above 8, nesting above
3, or more than 3 parameters produces a warning, following the repository's
coding conventions. These checks help readability but do not replace review
of naming, decomposition, or architecture.

Generated files, dependencies, vendored code, frozen experiment fixtures, and
experiment outputs are excluded. Python remains under its existing tooling.
This setup does not automatically reformat existing source. The repository
has existing lint and formatting violations, so the CI checks initially
report findings without blocking builds. After cleanup, remove
`continue-on-error` from both quality-check steps to make them blocking.

## Workspace guide

Each Nx project owns a README with its public contract, usage, and development
commands.

### Agent

- [`agents/doric`](agents/doric/README.md) — Direct REST and Socket.IO host,
  PostgreSQL persistence, and long-lived sandbox sessions.

### Built-in bundles

- [`bundles/core`](bundles/core/README.md) — filesystem, shell, and web tools
  plus general execution skills.
- [`bundles/git`](bundles/git/README.md) — structured Git execution and focused
  Git workflow skills.

### Libraries

- [`packages/agent`](packages/agent/README.md) — provider-neutral agent loop.
- [`packages/bundle`](packages/bundle/README.md) — strict runtime loader for
  bundle manifests, tools, and skills.
- [`packages/config`](packages/config/README.md) — parser for message-carried
  agent configuration.
- [`packages/docker`](packages/docker/README.md) — Docker implementation of the
  sandbox provider contract.
- [`packages/firecracker`](packages/firecracker/README.md) — direct
  Firecracker/KVM sandbox provider.
- [`packages/jsonl`](packages/jsonl/README.md) — streaming JSON Lines storage.
- [`packages/llms`](packages/llms/README.md) — model-provider integrations.
- [`packages/messages`](packages/messages/README.md) — provider-neutral message
  storage and normalization.
- [`packages/oauth`](packages/oauth/README.md) — OAuth 2 authorization-code and
  PKCE helpers.
- [`packages/okf`](packages/okf/README.md) — Open Knowledge Format bundle
  generator.
- [`packages/sandbox`](packages/sandbox/README.md) — provider-neutral isolated
  workspace contract and helpers.
- [`packages/sandpool`](packages/sandpool/README.md) — warmed FIFO pool of
  sandbox sessions.
- [`packages/session`](packages/session/README.md) — process-local keyed session
  store.
- [`packages/tool`](packages/tool/README.md) — tool definitions, binding,
  validation, and execution.

### Standalone tools

- [`tools/okf`](tools/okf/README.md) — read-only search over workspace OKF
  bundles.

## Work in the repository

Install dependencies once, then use Nx project names from the guide above:

```console
npm ci
npx nx show projects
npx nx show project <project>
npx nx test <project>
```

The project README lists any additional build, run, typecheck, or e2e targets
and their prerequisites.
