import type { StateMachineError } from '../classes/state-machine-error.js';

export type StateMachineErrorCode =
  | 'handler_failed'
  | 'invalid_handler_return'
  | 'missing_handler';

export type StateMachineErrorData<States extends string = string> = {
  readonly code: StateMachineErrorCode;
  readonly message: string;
  readonly state: States;
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

export type StateMachineFail<Failed> = {
  readonly type: 'fail';
  readonly error: Failed;
};

export type StateMachineAction<
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
> =
  | StateMachineTransition<States, ArtifactsByState>
  | StateMachineFinish<Finished>
  | StateMachineFail<Failed>;

export type StateMachineTransitionArguments<
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
> = {
  readonly [State in States]: readonly [
    state: State,
    artifacts: ArtifactsByState[State],
  ];
}[States];

export type StateMachineTransitionFunction<
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
> = (
  ...args: StateMachineTransitionArguments<States, ArtifactsByState>
) => StateMachineTransition<States, ArtifactsByState>;

export type StateMachineFinishFunction<Finished> = (
  value?: Finished,
) => StateMachineFinish<Finished>;

export type StateMachineFailFunction<Failed> = (
  error: Failed,
) => StateMachineFail<Failed>;

export type StateMachineHandlerScope<
  States extends string,
  State extends States,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
> = {
  readonly context: Context;
  readonly state: State;
  readonly transition: StateMachineTransitionFunction<States, ArtifactsByState>;
  readonly finish: StateMachineFinishFunction<Finished>;
  readonly fail: StateMachineFailFunction<Failed>;
};

export type StateMachineHandler<
  States extends string,
  State extends States,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
> = (
  artifacts: ArtifactsByState[State],
  scope: StateMachineHandlerScope<
    States,
    State,
    Context,
    ArtifactsByState,
    Finished,
    Failed
  >,
) =>
  | StateMachineAction<States, ArtifactsByState, Finished, Failed>
  | Promise<StateMachineAction<States, ArtifactsByState, Finished, Failed>>;

export type StateMachineHandlers<
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
> = {
  readonly [State in States]: StateMachineHandler<
    States,
    State,
    Context,
    ArtifactsByState,
    Finished,
    Failed
  >;
};

export type StateMachineRunInput<
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
> = {
  readonly [State in States]: {
    readonly context: Context;
    readonly state: State;
    readonly artifacts: ArtifactsByState[State];
  };
}[States];

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

export type StateMachineFailedResult<States extends string, Context, Failed> = {
  readonly status: 'failed';
  readonly error: Failed;
  readonly state: States;
  readonly context: Context;
};

export type StateMachineErrorResult<States extends string, Context> = {
  readonly status: 'error';
  readonly error: StateMachineError<States>;
  readonly state: States;
  readonly context: Context;
};

export type StateMachineResult<
  States extends string,
  Context,
  Finished,
  Failed,
> =
  | StateMachineFinishedResult<States, Context, Finished>
  | StateMachineFailedResult<States, Context, Failed>
  | StateMachineErrorResult<States, Context>;

export type StateMachineDefinition<
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
> = {
  readonly run: (
    input: StateMachineRunInput<States, Context, ArtifactsByState>,
  ) => Promise<StateMachineResult<States, Context, Finished, Failed>>;
};
