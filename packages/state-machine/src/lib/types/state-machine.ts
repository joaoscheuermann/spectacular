import type { StateMachineErrorObject } from '../classes/state-machine-error.js';

export type StateMachineStatus = 'idle' | 'running' | 'finished' | 'error';

export type StateMachineErrorCode =
  | 'concurrent_dispatch'
  | 'duplicate_state_registration'
  | 'error_listener_failed'
  | 'handler_failed'
  | 'invalid_handler_return'
  | 'missing_handler'
  | 'terminal_dispatch';

export type StateMachineErrorData<States extends string = string> = {
  readonly code: StateMachineErrorCode;
  readonly message: string;
  readonly state?: States;
};

export type StateMachineTransition<
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
> = {
  readonly [State in States]: {
    readonly type: 'transition';
    readonly state: State;
    readonly artifacts: ArtifactsByState[State];
  };
}[States];

export type StateMachineFinish<Finished> = {
  readonly type: 'finish';
  readonly value: Finished | undefined;
};

export type StateMachineAction<
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
> =
  | StateMachineTransition<States, ArtifactsByState>
  | StateMachineFinish<Finished>;

export type StateMachineDispatch<
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
> = <State extends States>(
  state: State,
  artifacts: ArtifactsByState[State],
) => StateMachineTransition<States, ArtifactsByState>;

export type StateMachineFinishFunction<Finished> = (
  value?: Finished,
) => StateMachineFinish<Finished>;

export type StateMachineHandlerScope<
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
> = {
  readonly context: Context;
  readonly state: States;
  readonly dispatch: StateMachineDispatch<States, ArtifactsByState>;
  readonly finish: StateMachineFinishFunction<Finished>;
};

export type StateMachineHandler<
  States extends string,
  State extends States,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
> = (
  artifacts: ArtifactsByState[State],
  scope: StateMachineHandlerScope<States, Context, ArtifactsByState, Finished>,
) =>
  | StateMachineAction<States, ArtifactsByState, Finished>
  | Promise<StateMachineAction<States, ArtifactsByState, Finished>>;

export type StateMachineFinishedResult<
  States extends string,
  Context,
  Finished,
> = {
  readonly status: 'finished';
  readonly value: Finished | undefined;
  readonly state: States;
  readonly context: Context;
};

export type StateMachineErrorResult<States extends string, Context> = {
  readonly status: 'error';
  readonly error: StateMachineErrorObject<States>;
  readonly state: States;
  readonly context: Context;
};

export type StateMachineResult<
  States extends string,
  Context,
  Finished,
> =
  | StateMachineFinishedResult<States, Context, Finished>
  | StateMachineErrorResult<States, Context>;

export type StateMachineTransitionListener<States extends string> = (
  from: States,
  to: States,
) => void;

export type StateMachineErrorListener<
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
> = (
  error: StateMachineErrorObject<States>,
  dispatch: StateMachineDispatch<States, ArtifactsByState>,
) =>
  | void
  | StateMachineTransition<States, ArtifactsByState>
  | Promise<void | StateMachineTransition<States, ArtifactsByState>>;

export type StateMachineFinishListener<
  States extends string,
  Context,
  Finished,
> = (result: StateMachineFinishedResult<States, Context, Finished>) => void;

export type StateMachine<
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
> = {
  readonly register: <State extends States>(
    state: State,
    handler: StateMachineHandler<
      States,
      State,
      Context,
      ArtifactsByState,
      Finished
    >,
  ) => StateMachine<States, Context, ArtifactsByState, Finished>;
  readonly dispatch: <State extends States>(
    state: State,
    artifacts: ArtifactsByState[State],
  ) => Promise<StateMachineResult<States, Context, Finished>>;
  readonly on: {
    (
      event: 'transition',
      listener: StateMachineTransitionListener<States>,
    ): () => void;
    (
      event: 'error',
      listener: StateMachineErrorListener<States, ArtifactsByState>,
    ): () => void;
    (
      event: 'finish',
      listener: StateMachineFinishListener<States, Context, Finished>,
    ): () => void;
  };
  readonly isDone: () => boolean;
  readonly status: () => StateMachineStatus;
  readonly result: () =>
    | StateMachineResult<States, Context, Finished>
    | undefined;
};
