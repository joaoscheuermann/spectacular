import {
  StateMachineErrorObject,
  stateMachineError,
} from './classes/state-machine-error.js';
import type {
  StateMachine,
  StateMachineAction,
  StateMachineErrorListener,
  StateMachineFinish,
  StateMachineFinishListener,
  StateMachineHandler,
  StateMachineResult,
  StateMachineStatus,
  StateMachineTransition,
  StateMachineTransitionListener,
} from './types/state-machine.js';

type HandlerMap<
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
> = Map<
  States,
  StateMachineHandler<States, States, Context, ArtifactsByState, Finished>
>;

type RunStep<
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
> = StateMachineTransition<States, ArtifactsByState>;

/** Creates an embeddable in-memory runner for typed agent-core state transitions. */
export const createStateMachine = <
  States extends string,
  Context,
  ArtifactsByState extends Record<States, unknown>,
  Finished = void,
>(
  context: Context,
): StateMachine<States, Context, ArtifactsByState, Finished> => {
  const handlers: HandlerMap<States, Context, ArtifactsByState, Finished> =
    new Map();
  const transitionListeners: StateMachineTransitionListener<States>[] = [];
  const errorListeners: StateMachineErrorListener<
    States,
    ArtifactsByState
  >[] = [];
  const finishListeners: StateMachineFinishListener<
    States,
    Context,
    Finished
  >[] = [];

  let currentStatus: StateMachineStatus = 'idle';
  let currentResult:
    | StateMachineResult<States, Context, Finished>
    | undefined;

  const dispatchAction = <State extends States>(
    state: State,
    artifacts: ArtifactsByState[State],
  ): StateMachineTransition<States, ArtifactsByState> => ({
    type: 'transition',
    state,
    artifacts,
  });

  const finishAction = (value?: Finished): StateMachineFinish<Finished> => ({
    type: 'finish',
    value,
  });

  const failBeforeRun = (code: 'concurrent_dispatch' | 'terminal_dispatch') => {
    const message =
      code === 'concurrent_dispatch'
        ? 'State machine is already running.'
        : 'State machine has already reached a terminal state.';

    throw stateMachineError<States>(code, message);
  };

  const recover = async (
    error: StateMachineErrorObject<States>,
  ): Promise<StateMachineTransition<States, ArtifactsByState> | undefined> => {
    for (const listener of errorListeners) {
      try {
        const action = await listener(error, dispatchAction);

        if (action !== undefined) {
          return isTransition<States, ArtifactsByState>(action)
            ? action
            : Promise.reject(
                stateMachineError(
                  'invalid_handler_return',
                  'Error listener returned an invalid transition action.',
                  error.data.state,
                ),
              );
        }
      } catch (cause) {
        return Promise.reject(
          stateMachineError(
            'error_listener_failed',
            'Error listener failed while handling a state-machine error.',
            error.data.state,
            cause,
          ),
        );
      }
    }

    return undefined;
  };

  const emitTransition = (from: States, to: States) => {
    transitionListeners.forEach((listener) => listener(from, to));
  };

  const run = async (
    initial: RunStep<States, ArtifactsByState>,
  ): Promise<StateMachineResult<States, Context, Finished>> => {
    let step = initial;

    while (true) {
      const state = step.state;
      const handler = handlers.get(state);

      if (handler === undefined) {
        const error = stateMachineError(
          'missing_handler',
          `No state handler registered for: ${state}`,
          state,
        );
        const recovered = await recover(error);

        if (recovered !== undefined) {
          emitTransition(state, recovered.state);
          step = recovered;
          continue;
        }

        return commitError(error, state);
      }

      let action: unknown;

      try {
        action = await handler(step.artifacts, {
          context,
          state,
          dispatch: dispatchAction,
          finish: finishAction,
        });
      } catch (cause) {
        const error = stateMachineError(
          'handler_failed',
          `State handler failed: ${state}`,
          state,
          cause,
        );
        const recovered = await recover(error);

        if (recovered !== undefined) {
          emitTransition(state, recovered.state);
          step = recovered;
          continue;
        }

        return commitError(error, state);
      }

      if (!isAction<States, ArtifactsByState, Finished>(action)) {
        const error = stateMachineError(
          'invalid_handler_return',
          `State handler returned an invalid action: ${state}`,
          state,
        );
        const recovered = await recover(error);

        if (recovered !== undefined) {
          emitTransition(state, recovered.state);
          step = recovered;
          continue;
        }

        return commitError(error, state);
      }

      if (action.type === 'finish') {
        return commitFinish(action, state);
      }

      emitTransition(state, action.state);
      step = action;
    }
  };

  const commitFinish = (
    action: StateMachineFinish<Finished>,
    state: States,
  ): StateMachineResult<States, Context, Finished> => {
    const result = {
      status: 'finished',
      value: action.value,
      state,
      context,
    } as const;

    currentStatus = 'finished';
    currentResult = result;
    finishListeners.forEach((listener) => listener(result));

    return result;
  };

  const commitError = (
    error: StateMachineErrorObject<States>,
    state: States,
  ): StateMachineResult<States, Context, Finished> => {
    const result = {
      status: 'error',
      error,
      state,
      context,
    } as const;

    currentStatus = 'error';
    currentResult = result;

    return result;
  };

  const machine: StateMachine<States, Context, ArtifactsByState, Finished> = {
    register(state, handler) {
      if (handlers.has(state)) {
        throw stateMachineError(
          'duplicate_state_registration',
          `State handler already registered for: ${state}`,
          state,
        );
      }

      handlers.set(
        state,
        handler as StateMachineHandler<
          States,
          States,
          Context,
          ArtifactsByState,
          Finished
        >,
      );

      return machine;
    },
    async dispatch(state, artifacts) {
      if (currentStatus === 'running') {
        failBeforeRun('concurrent_dispatch');
      }

      if (currentStatus === 'finished' || currentStatus === 'error') {
        failBeforeRun('terminal_dispatch');
      }

      currentStatus = 'running';

      try {
        return await run(dispatchAction(state, artifacts));
      } catch (cause) {
        const error =
          cause instanceof StateMachineErrorObject
            ? cause
            : stateMachineError(
                'handler_failed',
                'State-machine run failed.',
                state,
                cause,
              );

        return commitError(error, error.data.state ?? state);
      }
    },
    on(event, listener) {
      const listeners = listenersFor(
        event,
        transitionListeners,
        errorListeners,
        finishListeners,
      );

      listeners.push(listener as never);

      return () => {
        const index = listeners.indexOf(listener as never);

        if (index >= 0) {
          listeners.splice(index, 1);
        }
      };
    },
    isDone: () => currentStatus === 'finished' || currentStatus === 'error',
    status: () => currentStatus,
    result: () => currentResult,
  };

  return machine;
};

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
): value is StateMachineFinish<Finished> =>
  isRecord(value) && value.type === 'finish' && 'value' in value;

const isAction = <
  States extends string,
  ArtifactsByState extends Record<States, unknown>,
  Finished,
>(
  value: unknown,
): value is StateMachineAction<States, ArtifactsByState, Finished> =>
  isTransition(value) || isFinish(value);

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;

const listenersFor = (
  event: 'transition' | 'error' | 'finish',
  transitionListeners: unknown[],
  errorListeners: unknown[],
  finishListeners: unknown[],
): unknown[] => {
  if (event === 'transition') {
    return transitionListeners;
  }

  if (event === 'error') {
    return errorListeners;
  }

  return finishListeners;
};
