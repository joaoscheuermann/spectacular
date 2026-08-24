# Doric Direct agent

Doric exposes long-lived, sandbox-backed Direct sessions. PostgreSQL stores
configuration, public session state, provider-ready message history, and the
complete ordered Agent event stream. Socket.IO replays that durable history
before continuing with live delivery.

The HTTP and Socket.IO interfaces share one listener. It defaults to
`0.0.0.0:3000` and can be changed with `DORIC_HOST` and `DORIC_PORT`.

## Session lifecycle

`POST /sessions` creates a `queued` session without a prompt and starts
acquiring one sandbox lease. Acquisition changes the state to `ready`. The
lease and sandbox are reused by every prompt and remain reserved until the
session is terminated or the process shuts down.

Accepted prompts are serialized in FIFO order. Each prompt changes the state
from `ready` to `running` and back to `ready`, whether it succeeds or fails. A
prompt failure does not terminate the session. Termination changes an active
session through `cancelling` to `cancelled`; sandbox acquisition or restart
reconciliation can change it to `failed`.

There is no automatic expiry. At most ten sandboxes are provisioned by the
default pool, so callers must terminate sessions they no longer need.

For every prompt Doric creates a fresh Agent with:

- the captured `models.execution` provider/model and `execution.maxTurns`;
- all bundle tools bound to the session sandbox;
- a fresh tool-call storage;
- message storage initialized from the exact persisted conversation history;
- one deterministic system prompt followed by every bundle skill body in
  bundle order.

Complete and partial message history is persisted after success or failure and
becomes the next prompt's history.

## REST API

| Method   | Path                      | Success | Purpose                                      |
| -------- | ------------------------- | ------- | -------------------------------------------- |
| `GET`    | `/config`                 | `200`   | Return the active configuration snapshot.    |
| `PUT`    | `/config`                 | `200`   | Replace the complete configuration.          |
| `POST`   | `/sessions`               | `202`   | Create a prompt-free queued session.         |
| `GET`    | `/sessions`               | `200`   | List sessions with cursor pagination.        |
| `GET`    | `/sessions/:id`           | `200`   | Return the same representation as the list.  |
| `POST`   | `/sessions/:id/prompt`    | `202`   | Accept one prompt into the FIFO queue.       |
| `GET`    | `/sessions/:id/events`    | `200`   | Replay all or cursor-filtered events.        |
| `GET`    | `/sessions/:id/ssh`       | `200`   | Poll SSH access for the reserved sandbox.    |
| `POST`   | `/sessions/:id/terminate` | `200`   | Request idempotent cancellation.             |
| `DELETE` | `/sessions/:id`           | `204`   | Delete a terminal session and its events.    |
| `GET`    | `/vms`                    | `200`   | List provisioned runtimes.                   |
| `GET`    | `/vms/:id/ssh`            | `200`   | Return SSH access for a currently leased VM. |

### Configuration

The `GET/PUT /config` schema stores provider URLs, credential
environment-variable names, one `models.execution` profile, and
`execution.maxTurns`. Direct uses that execution profile for every prompt in a
session.

Credential values are resolved from the named environment variables at
runtime. They are never stored in the configuration tables.

### Create and prompt

```console
curl -sS -X POST http://127.0.0.1:3000/sessions
```

```json
{
  "id": "018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601",
  "state": "queued",
  "configRevision": 1,
  "lastSequence": 0,
  "createdAt": "2026-08-24T12:00:00.000Z",
  "updatedAt": "2026-08-24T12:00:00.000Z",
  "ssh": {
    "href": "/sessions/018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601/ssh"
  }
}
```

```console
curl -sS -X POST \
  -H 'content-type: application/json' \
  -d '{"prompt":"Inspect the repository and run the focused tests."}' \
  http://127.0.0.1:3000/sessions/018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601/prompt
```

```json
{
  "promptId": "018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1602"
}
```

Sessions accept prompts while `queued`, `ready`, or `running`. Concurrent
requests receive independent prompt IDs and enter the same FIFO queue. Terminal
or cancelling sessions return `409 session_inactive`.

The public list/detail representation contains `id`, `state`,
`configRevision`, `lastSequence`, timestamps, and `errorCode` when applicable.
It never includes prompts, messages, events, or results.

### Event replay

`GET /sessions/:id/events` returns the complete point-in-time history.
`afterSequence=N` is an optional exclusive cursor. The response prohibits
caching and has this shape:

```json
{
  "events": [
    {
      "sessionId": "018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601",
      "promptId": "018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1602",
      "sequence": 1,
      "type": "prompt.accepted",
      "event": { "type": "prompt.accepted" },
      "createdAt": "2026-08-24T12:00:01.000Z"
    }
  ],
  "lastSequence": 1
}
```

Agent stream events retain reasoning, provider replay, tool payloads/results,
usage, response finishes, and failures. Doric also writes `prompt.accepted`,
`agent.failed`, and `agent.cancelled`. Events are persisted before publication.

Errors preserve defined `name`, `message`, `stack`, `cause`, and own
properties. Undefined object properties are omitted; undefined array entries,
cycles, and other non-JSON values use explicit markers. Values of configured
credentials are replaced with `[REDACTED]` before persistence.

## Socket.IO

Connect to namespace `/sessions` with `sessionId` in the connection query.
`afterSequence` is optional; omitting it requests full playback.

```ts
import { io } from 'socket.io-client';

const socket = io('http://127.0.0.1:3000/sessions', {
  query: { sessionId, afterSequence: 42 },
});
```

The server emits:

| Event              | Payload                                        |
| ------------------ | ---------------------------------------------- |
| `session:snapshot` | `{ sessionId, session, events }`               |
| `agent:event`      | One complete persisted session-event envelope. |
| `session:updated`  | The current public session representation.     |
| `session:deleted`  | `{ sessionId }`                                |

The subscription is registered before PostgreSQL replay. Events published
during replay are buffered and deduplicated by sequence before live delivery.

## Persistence

The Prisma schema uses generic `Session` and `SessionEvent` models. Messages
and event bodies are JSONB, event sequences are contiguous per session, and
deleting a terminal session cascades to its events.

Migration `20260824000000_direct_sessions` creates the complete Direct
configuration, session, and event schema from an empty PostgreSQL database.

Production startup never runs migrations implicitly. Apply them separately:

```console
npx nx run doric:migrate
```

## Security

The API is unauthenticated and binds to all interfaces by default. Keep it on
an isolated trusted network. This is especially important because event replay
intentionally exposes model reasoning, replay data, tool input/output, stacks,
and causes. Full event bodies and prompts are not written to Doric operational
logs.
