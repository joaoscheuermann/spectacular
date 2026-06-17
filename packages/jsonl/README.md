# jsonl

Stream-backed helpers for appending and reading JSON Lines files.

```ts
import { jsonl } from 'jsonl';

const file = jsonl('path/to/events.jsonl');

await file.append({ type: 'started' });
await file.append({ type: 'finished', ok: true });
await file.close();

for await (const record of file.read()) {
  console.log(record);
}
```

`append` serializes one JSON value per line and preserves append call order.
`read` streams the file line by line and yields parsed values.

## Building

Run `nx build jsonl` to build the library.

## Testing

Run `nx test jsonl` to compile and run the package tests.
