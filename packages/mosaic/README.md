# mosaic

Goal-oriented planning and preparation workflow extracted from the Doric host.

```ts
import mosaic, { mosaic as createMosaic } from 'mosaic';
```

The factory receives provider, logger, model, skill, tool, and vector database
dependencies. It has no session option.

Mosaic owns the workflow policy and uses `state-machine` only for reusable,
run-local typed transition execution. Its fixed lifecycle is
`graph -> schedule -> bundle -> execution -> schedule`: an already-completed
graph finishes successfully, while a graph that cannot produce another ready
node preserves the intentional missing-ready domain failure. The current
workflow stores graphs as a run-local LIFO stack, appends decomposed graphs,
and mutates the active graph at the end of the stack when scheduling nodes.
Each handler passes the graph stack explicitly to the next transition.

Execution creates one `agent` per ready node. It gives the agent an isolated
in-memory message storage, a Markdown system prompt constructed from that
node's selected skills and tools, and the executable tools resolved by name
from the Mosaic tool catalog. Nodes in a wave execute concurrently, moving
from `ready` to `running` and then `completed`; a failed agent marks its node
`failed` and fails the workflow. Execution logs node IDs and selected skill and
tool names, never goals, prompts, responses, or error details.
