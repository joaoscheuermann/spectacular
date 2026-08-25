# session

`session` provides process-local, in-memory storage keyed by caller-owned
string IDs.

## Use

```ts
import { createSessionStore } from 'session';

type Workspace = { readonly path: string };
const sessions = createSessionStore<Workspace>();

const workspace = await sessions.getOrCreate('request-123', async () => ({
  path: await provisionWorkspace(),
}));

console.log(sessions.get('request-123'));
console.log(sessions.list());
sessions.delete('request-123');
```

Concurrent `getOrCreate` calls for the same missing ID share one in-flight
factory promise. `get` reads only a completed value; `load` returns either the
completed value or the in-flight promise. `delete` and `clear` remove tracked
entries, while `list` returns completed entries in insertion order.

This package provides no persistence, expiry, cross-process coordination, or
factory cancellation. A host owns those policies and any cleanup required for
stored values.

## Development

```console
npx nx build session
npx nx test session
```
