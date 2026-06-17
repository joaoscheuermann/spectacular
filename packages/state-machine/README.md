# state-machine

Typed in-memory state-machine control flow for the TypeScript agent core.

```ts
import { createStateMachine } from 'state-machine';

type State = 'draft' | 'review' | 'complete';
type Artifacts = {
  draft: { prompt: string };
  review: { summary: string };
  complete: { answer: string };
};

const machine = createStateMachine<State, { runId: string }, Artifacts, string>({
  runId: 'run_1',
});

machine
  .register('draft', async (artifacts, { dispatch }) =>
    dispatch('review', { summary: artifacts.prompt.trim() }),
  )
  .register('review', (artifacts, { dispatch }) =>
    dispatch('complete', { answer: artifacts.summary }),
  )
  .register('complete', (artifacts, { finish }) => finish(artifacts.answer));

machine.on('transition', (from, to) => {
  console.log(`${from} -> ${to}`);
});

machine.on('error', (error, dispatch) => {
  if (error.data.code === 'handler_failed') {
    return dispatch('complete', { answer: 'recovered' });
  }
});

const result = await machine.dispatch('draft', { prompt: 'hello' });
```

`createStateMachine` stores handlers and listeners in memory only. It does not
persist state, run external services, or provide a workflow framework. Public
`dispatch(state, artifacts)` starts one run at a time, and the artifact payload
is checked against the state-specific artifact map at compile time.

Handlers return either `dispatch(nextState, artifacts)` to continue or
`finish(value?)` to close the run. Initial public dispatch does not emit a
transition event; handler-returned transitions emit `transition(from, to)`
before the next handler runs.

Runtime inspection is available through `status()`, `isDone()`, and `result()`.
Duplicate registrations, concurrent dispatch, dispatch after terminal
completion, missing handlers, invalid handler returns, and unrecovered handler
failures use `StateMachineErrorObject`.

## Building

Run `nx build state-machine` to build the library.

## Testing

Run `nx test state-machine` to compile and run the package tests.
