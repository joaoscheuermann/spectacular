# state-machine

Typed, reusable state-machine definitions for the TypeScript agent core.

```ts
import { createStateMachine } from 'state-machine';

type State = 'draft' | 'review' | 'complete';
type Context = {
  readonly cancelled: boolean;
};
type Artifacts = {
  readonly draft: { readonly prompt: string };
  readonly review: { readonly summary: string };
  readonly complete: { readonly answer: string };
};
type Failure = {
  readonly code: 'cancelled';
};

const definition = createStateMachine<
  State,
  Context,
  Artifacts,
  string,
  Failure
>({
  draft: async (artifacts, { transition }) =>
    transition('review', { summary: artifacts.prompt.trim() }),
  review: (artifacts, { context, transition, fail }) =>
    context.cancelled
      ? fail({ code: 'cancelled' })
      : transition('complete', { answer: artifacts.summary }),
  complete: (artifacts, { finish }) => finish(artifacts.answer),
});

const result = await definition.run({
  context: { cancelled: false },
  state: 'draft',
  artifacts: { prompt: 'hello' },
});
```

The handler map is exhaustive. Each handler receives artifacts for its own
state and a scope containing exactly `context`, `state`, `transition`,
`finish`, and `fail`. Both initial run artifacts and transition artifacts are
correlated with their state at compile time.

A definition stores only its handler map. Every `run` keeps its context,
current state, and artifacts local to that call, so the same definition can be
run repeatedly or concurrently.

## Results

`run` resolves one of three results:

- `status: 'finished'` contains the value passed to `finish`, the terminal
  state, and the run context.
- `status: 'failed'` contains the exact domain value passed to `fail`, the
  terminal state, and the run context.
- `status: 'error'` contains a `StateMachineError`, the failing state, and the
  run context.

Engine errors use only `handler_failed`, `invalid_handler_return`, and
`missing_handler`. A value thrown by a handler is preserved unchanged at
`result.error.cause` on a `handler_failed` result.

The package is process-local and has no persistence, listeners, recovery
hooks, or external side effects.

## Testing

Run `npx nx test state-machine` to compile and run the package tests.
