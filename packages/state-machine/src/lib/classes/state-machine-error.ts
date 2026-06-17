import type {
  StateMachineErrorData,
  StateMachineErrorCode,
} from '../types/state-machine.js';

/** Error subclass used for structured state-machine lifecycle failures. */
export class StateMachineErrorObject<
  States extends string = string,
> extends Error {
  readonly data: StateMachineErrorData<States>;

  constructor(
    data: StateMachineErrorData<States>,
    options?: ErrorOptions,
  ) {
    super(data.message, options);
    this.name = 'StateMachineErrorObject';
    this.data = data;
  }
}

export const stateMachineError = <States extends string>(
  code: StateMachineErrorCode,
  message: string,
  state?: States,
  cause?: unknown,
): StateMachineErrorObject<States> =>
  new StateMachineErrorObject(
    {
      code,
      message,
      ...(state === undefined ? {} : { state }),
    },
    cause === undefined ? undefined : { cause },
  );
