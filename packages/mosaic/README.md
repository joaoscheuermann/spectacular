# mosaic

Incomplete goal-oriented workflow extracted from the Doric host.

```ts
import mosaic, { mosaic as createMosaic } from 'mosaic';
```

The factory receives provider, logger, model, skill, tool, vector database, and
session-placeholder dependencies. Its `prompt(input)` method currently plans
and selects node menus but intentionally does not execute nodes or tools.
