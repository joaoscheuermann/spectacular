# mosaic

Goal-oriented planning and preparation workflow extracted from the Doric host.

```ts
import mosaic, { mosaic as createMosaic } from 'mosaic';
```

The factory receives provider, logger, model, skill, tool, and vector database
dependencies. It has no session option.

## Lifecycle

Mosaic owns the workflow policy and uses `state-machine` only for reusable,
run-local typed transition execution. Its fixed lifecycle is
`graph -> schedule -> bundle -> execution -> schedule`: an already-completed
graph finishes successfully, while a graph that cannot produce another ready
node preserves the intentional missing-ready domain failure. The current
workflow stores graphs as a run-local LIFO stack, appends decomposed graphs,
and mutates the active graph at the end of the stack when scheduling nodes.
Each handler passes the graph stack explicitly to the next transition.

## Execution

Execution creates one `agent` per ready node with isolated in-memory message
storage and executable tools resolved by name from the Mosaic tool catalog.
Each node makes one `agent.complete` call with the provider's tool definitions
and a node-bound structured outcome schema. Its collision-safe Markdown input
contains the original request, current goal and ordered completion criteria,
artifacts from transitive ancestor nodes only, ordered selected skill bodies,
and the available tool names and descriptions; tool schemas are not duplicated
in the prompt.

For nodes with executable tools, `agent` represents the outcome schema as a
strict terminal tool. Provider requests therefore contain ordinary tools plus
that terminal tool, without a provider-native structured-output field. The
terminal call is validated by `agent` and is not executed as a Mosaic tool.

The terminal outcome is `completed`, `needs_revision`, `blocked`, or `failed`.
It evaluates every completion criterion in order and may reference only tool
call IDs observed in that node's isolated message history. A completed outcome
stores its Markdown result as a `text/markdown` artifact, appends additional
artifacts, marks the node completed, and returns the fully completed wave to
`schedule`. Other terminal outcomes retain their status and fail the current
wave; localized graph revision remains outside the current implementation.
Provider, tool, schema, and observation-validation failures mark the node
failed. Execution logs node IDs, terminal statuses, and selected skill and tool
names, never goals, prompts, outcomes, reasons, tool payloads, or error details.

## Correspondence with the MOSAIC paper

The normative source is [MOSAIC Core Profile 0.1](docs/MOSAIC_arquitetura_core.html).
The implementation comments use its section and PDF page numbers. The following
short excerpts identify the contract behind the execution code:

| Runtime concept                | Paper definition                                                     | Short excerpt                                                                                                                                     |
| ------------------------------ | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| Projected node context         | [Section 4.7, p. 15](docs/MOSAIC_arquitetura_core.html#15)           | "O executor recebe somente o contexto necessário para o objetivo atual."                                                                          |
| Model/tool loop                | [Section 4.8, p. 16](docs/MOSAIC_arquitetura_core.html#16)           | "O modelo pode concluir diretamente ou chamar tools. Uma observação volta ao mesmo nó."                                                           |
| Node lifecycle                 | [Section 4.9, pp. 16-17](docs/MOSAIC_arquitetura_core.html#16)       | "O scheduler o coloca em running. A satisfação de doneWhen leva a completed."                                                                     |
| Observation and outcome fields | [Appendix A, table A.2, p. 30](docs/MOSAIC_arquitetura_core.html#30) | `Observation` correlates `toolName`, `callId`, input and output; `NodeOutcome` carries status, result, observations, revision request and reason. |
| Scheduler/executor order       | [Algorithm 1, pp. 18-19](docs/MOSAIC_arquitetura_core.html#18)       | Select a pending goal whose dependencies are completed, run it to a terminal outcome, incorporate its result or observations, then continue.      |
| Final assembly                 | [Section 4.10, p. 17](docs/MOSAIC_arquitetura_core.html#17)          | "compor_resposta não chama o modelo e não aplica skills novamente."                                                                               |

The paper specifies semantic contracts, not provider transport or dispatch
mechanics. Concurrent ready waves, `Promise.allSettled`, one isolated `agent`
instance per node, the strict terminal-output tool, explicit per-criterion proof
entries, and the `text/markdown` artifact representation are Doric choices. As
Algorithm 1 notes, conforming implementations may vary in model, language,
serialization format, and tool-calling protocol while preserving the Core
invariants and Appendix A contracts.
