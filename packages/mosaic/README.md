# mosaic

Goal-oriented planning and preparation workflow extracted from the Doric host.

```ts
import mosaic, { mosaic as createMosaic } from 'mosaic';
```

The factory receives provider, logger, model, skill, tool, and vector database
dependencies. It has no session option.

Mosaic owns the workflow policy and uses `state-machine` only for reusable,
run-local typed transition execution. Its fixed lifecycle is
`graph -> schedule -> bundle -> schedule`: an already-completed graph
finishes successfully, while a graph that cannot produce another ready node
preserves the intentional missing-ready domain failure. The current workflow
stores graphs as a run-local LIFO stack, appends decomposed graphs, and mutates
the active graph at the end of the stack when scheduling nodes. Each handler
passes the graph stack explicitly to the next transition. Mosaic plans and
prepares per-node skill and tool menus but does not execute nodes or tools.
