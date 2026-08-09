# Doric host

Doric stores Mosaic configuration, sessions, and ordered events in PostgreSQL.
Migrations are deployed explicitly before the HTTP host starts.

## Network API

The HTTP and Socket.IO interfaces use the same origin. The host defaults to
`0.0.0.0` and the port defaults to `3000`; set `DORIC_HOST` or `DORIC_PORT` to
override them. All interfaces are intentionally unauthenticated. Configuration
updates can change provider base URLs and credential environment names, so
expose Doric only inside a trusted network.

### REST endpoints

| Method   | Path                             | Success | Description                                                  |
| -------- | -------------------------------- | ------- | ------------------------------------------------------------ |
| `GET`    | `/vms`                           | `200`   | Lists the currently running Docker or Firecracker sandboxes. |
| `GET`    | `/mosaic/config`                 | `200`   | Returns the active Mosaic configuration snapshot.            |
| `PUT`    | `/mosaic/config`                 | `200`   | Validates and completely replaces the Mosaic configuration.  |
| `POST`   | `/mosaic/sessions`               | `202`   | Creates and asynchronously starts one Mosaic session.        |
| `GET`    | `/mosaic/sessions`               | `200`   | Lists sessions from newest to oldest with cursor pagination. |
| `POST`   | `/mosaic/sessions/:id/terminate` | `200`   | Requests best-effort, idempotent session cancellation.       |
| `DELETE` | `/mosaic/sessions/:id`           | `204`   | Deletes a terminal session and its persisted events.         |

There is currently no health endpoint or single-session
`GET /mosaic/sessions/:id` endpoint.

#### `GET /vms`

Returns an array containing only sandboxes that are currently provisioned:

```json
[
  {
    "id": "sandbox-id",
    "provider": "docker"
  }
]
```

`provider` is either `docker` or `firecracker`.

#### `GET /mosaic/config`

Returns the active configuration, its monotonically increasing revision, and
the activation timestamp:

```json
{
  "configuration": {
    "providers": [],
    "models": {},
    "routing": {},
    "execution": {},
    "revision": {}
  },
  "revision": 1,
  "updatedAt": "2026-08-09T12:00:00.000Z"
}
```

Credential values are never returned. Provider entries contain only the
credential environment-variable name in `apiKeyEnv`.

#### `PUT /mosaic/config`

The request body is the complete `configuration` object returned by the GET,
without the outer `configuration`, numeric `revision`, or `updatedAt` fields:

```json
{
  "providers": [
    {
      "id": "openrouter",
      "baseUrl": "https://openrouter.ai/api/v1",
      "apiKeyEnv": "OPENROUTER_API_KEY"
    }
  ],
  "models": {
    "planning": {
      "providerId": "openrouter",
      "model": "qwen/qwen3.7-flash",
      "effort": "low"
    },
    "revision": {
      "providerId": "openrouter",
      "model": "google/gemini-3.6-flash",
      "effort": "low"
    },
    "execution": {
      "providerId": "openrouter",
      "model": "deepseek/deepseek-v4-flash-0731",
      "effort": "low"
    },
    "reranker": {
      "providerId": "openrouter",
      "model": "voyageai/rerank-2.5-lite"
    },
    "embedder": {
      "providerId": "openrouter",
      "model": "voyageai/voyage-4-large",
      "dimensions": 2048
    }
  },
  "routing": {
    "maxHintCandidates": 5,
    "maxRetrievedCandidates": 5,
    "maxSkills": 5
  },
  "execution": { "maxTurns": 32 },
  "revision": { "max": 3 }
}
```

The response is the same snapshot shape as `GET /mosaic/config`. Invalid
input returns `422 invalid_config`; failure while preparing or activating the
new generation returns `503 configuration_rejected`. Existing sessions retain
their captured configuration revision.

#### `POST /mosaic/sessions`

Creates a queued session and returns its session representation with status
`202`:

```json
{
  "prompt": "Implement the requested change."
}
```

The prompt must be a non-empty string. Invalid input returns
`422 invalid_prompt`.

#### `GET /mosaic/sessions`

The optional query parameters are:

| Parameter | Default | Constraint                              |
| --------- | ------- | --------------------------------------- |
| `limit`   | `50`    | Positive integer with a maximum of 100. |
| `cursor`  | none    | Session UUID returned as `nextCursor`.  |

The response is:

```json
{
  "sessions": [],
  "nextCursor": "018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601"
}
```

`nextCursor` is omitted when no additional page exists. Invalid pagination
returns `400 invalid_page`.

#### Session representation

Session creation, listing, termination, snapshots, and update events use this
shape:

```json
{
  "id": "018f47d2-e3b1-7b4f-8b2c-1f5a7fdf1601",
  "prompt": "Implement the requested change.",
  "state": "running",
  "configRevision": 1,
  "result": null,
  "lastSequence": 3,
  "createdAt": "2026-08-09T12:00:00.000Z",
  "updatedAt": "2026-08-09T12:00:01.000Z",
  "startedAt": "2026-08-09T12:00:01.000Z"
}
```

States are `queued`, `running`, `cancelling`, `completed`, `failed`, or
`cancelled`. `startedAt`, `finishedAt`, and `errorCode` are present only when
applicable.

#### `POST /mosaic/sessions/:id/terminate`

Requests cancellation and returns the latest session representation. Repeated
requests are safe. An invalid UUID returns `400 invalid_session_id`; an unknown
UUID returns `404 session_not_found`.

#### `DELETE /mosaic/sessions/:id`

Deletes only a `completed`, `failed`, or `cancelled` session. Its events are
removed by cascade. An active session returns `409 session_active`, an invalid
UUID returns `400 invalid_session_id`, and an unknown UUID returns
`404 session_not_found`.

All REST errors use a sanitized envelope:

```json
{
  "error": {
    "code": "error_code",
    "message": "Safe human-readable message."
  }
}
```

Unexpected failures return `500 internal_error`.

### Socket.IO namespace `/mosaic`

Connect Socket.IO to the `/mosaic` namespace on the same HTTP origin. The
database is the replay source of truth, and event sequence numbers are
monotonic within each session.

Client-to-server events:

| Event                 | Payload                         | Description                                 |
| --------------------- | ------------------------------- | ------------------------------------------- |
| `session:subscribe`   | `{ sessionId, afterSequence? }` | Starts snapshot, replay, and live delivery. |
| `session:unsubscribe` | `{ sessionId }`                 | Stops delivery for that session.            |

`sessionId` must be a UUID. `afterSequence` is an optional non-negative integer
and defaults to `0`. Invalid client event payloads are ignored.

Server-to-client events:

| Event              | Payload                          | Description                                     |
| ------------------ | -------------------------------- | ----------------------------------------------- |
| `session:snapshot` | `{ sessionId, session, events }` | Current session plus events after the sequence. |
| `mosaic:event`     | `{ sessionId, event }`           | One persisted Mosaic schema-version 2 event.    |
| `session:updated`  | Session representation           | Latest state after a session transition.        |
| `session:deleted`  | `{ sessionId }`                  | Indicates deletion and ends the subscription.   |

For an unknown session, `session:snapshot.session` is `null`. Each Mosaic event
contains its own `sequence`; clients should use it to order and deduplicate
events and pass the last received value as `afterSequence` after reconnecting.

## Running locally

Create a root `.env` from `.example.env`, set the PostgreSQL fields and runtime
provider key, then start the Docker sandbox profile from the repository root:

```sh
docker compose --env-file .env -f agents/doric/compose.yaml --profile docker up --build
```

On a Linux x86_64 host with KVM, use the Firecracker sandbox profile instead:

```sh
docker compose --env-file .env -f agents/doric/compose.yaml --profile firecracker up --build
```

For a host process outside Compose, apply migrations and start Doric with the
same `DORIC_DATABASE_URL`:

```sh
npx nx run doric:migrate
npx nx run doric:serve
```
