import { StateMachineError } from './classes/state-machine-error.js';
import type {
  StateMachineAction,
  StateMachineDefinition,
  StateMachineErrorCode,
  StateMachineErrorData,
  StateMachineErrorResult,
  StateMachineFailFunction,
  StateMachineFinishFunction,
  StateMachineHandler,
  StateMachineHandlers,
  StateMachineHandlerScope,
  StateMachineResult,
  StateMachineRunInput,
  StateMachineTransition,
  StateMachineTransitionArguments,
  StateMachineTransitionFunction,
} from './types/state-machine.js';

type RuntimeHandler<
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
> = StateMachineHandler<
  States,
  States,
  Context,
  ArtifactsByState,
  Finished,
  Failed
>;

type RuntimeHandlers<
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
> = Partial<
  Record<
    States,
    RuntimeHandler<States, Context, ArtifactsByState, Finished, Failed>
  >
>;

type Actions<
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
> = {
  readonly transition: StateMachineTransitionFunction<States, ArtifactsByState>;
  readonly finish: StateMachineFinishFunction<Finished>;
  readonly fail: StateMachineFailFunction<Failed>;
};

type HandlerCall =
  | { readonly type: 'returned'; readonly action: unknown }
  | { readonly type: 'threw'; readonly cause: unknown };

/** Creates a reusable definition whose run state is isolated to each call. */
export const createStateMachine = <
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished = void,
  Failed = never,
>(
  handlers: StateMachineHandlers<
    States,
    Context,
    ArtifactsByState,
    Finished,
    Failed
  >,
): StateMachineDefinition<
  States,
  Context,
  ArtifactsByState,
  Finished,
  Failed
> => {
  const runtimeHandlers = { ...handlers } as RuntimeHandlers<
    States,
    Context,
    ArtifactsByState,
    Finished,
    Failed
  >;
  const actions = createActions<States, ArtifactsByState, Finished, Failed>();

  return {
    run: (input) => execute(input, runtimeHandlers, actions),
  };
};

const createActions = <
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
>(): Actions<States, ArtifactsByState, Finished, Failed> => {
  const transition: StateMachineTransitionFunction<States, ArtifactsByState> = (
    ...args: StateMachineTransitionArguments<States, ArtifactsByState>
  ) => {
    const [state, artifacts] = args;

    return {
      type: 'transition',
      state,
      artifacts,
    } as StateMachineTransition<States, ArtifactsByState>;
  };
  const finish: StateMachineFinishFunction<Finished> = (value) => ({
    type: 'finish',
    value,
  });
  const fail: StateMachineFailFunction<Failed> = (error) => ({
    type: 'fail',
    error,
  });

  return { transition, finish, fail };
};

const execute = async <
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
>(
  input: StateMachineRunInput<States, Context, ArtifactsByState>,
  handlers: RuntimeHandlers<
    States,
    Context,
    ArtifactsByState,
    Finished,
    Failed
  >,
  actions: Actions<States, ArtifactsByState, Finished, Failed>,
): Promise<StateMachineResult<States, Context, Finished, Failed>> => {
  let step = initialStep(input);

  while (true) {
    const state = step.state;
    const handler = ownHandler(handlers, state);

    if (handler === undefined) {
      return engineError(
        issue(
          'missing_handler',
          `No state handler defined for: ${state}`,
          state,
        ),
        input.context,
      );
    }

    const call = await callHandler(handler, step.artifacts, {
      context: input.context,
      state,
      ...actions,
    });

    if (call.type === 'threw') {
      return engineError(
        issue('handler_failed', `State handler failed: ${state}`, state),
        input.context,
        { cause: call.cause },
      );
    }

    if (!isAction<States, ArtifactsByState, Finished, Failed>(call.action)) {
      return engineError(
        issue(
          'invalid_handler_return',
          `State handler returned an invalid action: ${state}`,
          state,
        ),
        input.context,
      );
    }

    if (call.action.type === 'finish') {
      return {
        status: 'finished',
        value: call.action.value,
        state,
        context: input.context,
      };
    }

    if (call.action.type === 'fail') {
      return {
        status: 'failed',
        error: call.action.error,
        state,
        context: input.context,
      };
    }

    step = call.action;
  }
};

const initialStep = <
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
>(
  input: StateMachineRunInput<States, Context, ArtifactsByState>,
): StateMachineTransition<States, ArtifactsByState> =>
  ({
    type: 'transition',
    state: input.state,
    artifacts: input.artifacts,
  }) as StateMachineTransition<States, ArtifactsByState>;

const ownHandler = <
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
>(
  handlers: RuntimeHandlers<
    States,
    Context,
    ArtifactsByState,
    Finished,
    Failed
  >,
  state: States,
):
  | RuntimeHandler<States, Context, ArtifactsByState, Finished, Failed>
  | undefined =>
  Object.prototype.hasOwnProperty.call(handlers, state) &&
  typeof handlers[state] === 'function'
    ? handlers[state]
    : undefined;

const callHandler = async <
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
>(
  handler: RuntimeHandler<States, Context, ArtifactsByState, Finished, Failed>,
  artifacts: ArtifactsByState[States],
  scope: StateMachineHandlerScope<
    States,
    States,
    Context,
    ArtifactsByState,
    Finished,
    Failed
  >,
): Promise<HandlerCall> => {
  try {
    return { type: 'returned', action: await handler(artifacts, scope) };
  } catch (cause) {
    return { type: 'threw', cause };
  }
};

const issue = <States extends string>(
  code: StateMachineErrorCode,
  message: string,
  state: States,
): StateMachineErrorData<States> => ({ code, message, state });

const engineError = <States extends string, Context>(
  data: StateMachineErrorData<States>,
  context: Context,
  options?: ErrorOptions,
): StateMachineErrorResult<States, Context> => ({
  status: 'error',
  error: new StateMachineError(data, options),
  state: data.state,
  context,
});

const isTransition = <
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
>(
  value: unknown,
): value is StateMachineTransition<States, ArtifactsByState> =>
  isRecord(value) &&
  value.type === 'transition' &&
  typeof value.state === 'string' &&
  'artifacts' in value;

const isFinish = <Finished>(
  value: unknown,
): value is {
  readonly type: 'finish';
  readonly value: Finished | undefined;
} => isRecord(value) && value.type === 'finish' && 'value' in value;

const isFail = <Failed>(
  value: unknown,
): value is {
  readonly type: 'fail';
  readonly error: Failed;
} => isRecord(value) && value.type === 'fail' && 'error' in value;

const isAction = <
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
  Failed,
>(
  value: unknown,
): value is StateMachineAction<States, ArtifactsByState, Finished, Failed> =>
  isTransition(value) || isFinish<Finished>(value) || isFail<Failed>(value);

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;
