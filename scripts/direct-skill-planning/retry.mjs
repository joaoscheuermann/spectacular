import { ProviderErrorObject } from 'llms';

export const providerRetryDelaysMs = Object.freeze([
  5_000, 10_000, 20_000, 30_000,
]);
export const providerAttempts = providerRetryDelaysMs.length + 1;

class ProviderCallError extends Error {
  constructor(failure) {
    super('Provider operation failed.');
    this.name = 'ProviderCallError';
    this.failure = failure;
  }
}

const retryDelay = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

const callContext = ({
  operation,
  model,
  caseName,
  round,
  arm,
  retrieval,
  goalIndex,
  skill,
  pair,
  orientation,
} = {}) => ({
  ...(operation === undefined ? {} : { operation }),
  ...(model === undefined ? {} : { model }),
  ...(caseName === undefined ? {} : { case: caseName }),
  ...(round === undefined ? {} : { round }),
  ...(arm === undefined ? {} : { arm }),
  ...(retrieval === undefined ? {} : { retrieval }),
  ...(goalIndex === undefined ? {} : { goalIndex }),
  ...(skill === undefined ? {} : { skill }),
  ...(pair === undefined ? {} : { pair }),
  ...(orientation === undefined ? {} : { orientation }),
});

const failureDetails = (error, context) => ({
  ...callContext(context),
  provider: error.data.provider,
  code: error.data.code,
  ...(error.data.status === undefined ? {} : { httpStatus: error.data.status }),
  ...(error.data.retryable === undefined
    ? {}
    : { retryable: error.data.retryable }),
});

export const retryProvider = async (
  operation,
  { onFailure, ...context } = {},
) => {
  let attempt = 1;

  while (true) {
    try {
      return await operation();
    } catch (error) {
      if (!(error instanceof ProviderErrorObject)) throw error;

      const failure = failureDetails(error, context);
      const exhausted =
        error.data.retryable === false || attempt === providerAttempts;
      const nextDelayMs = exhausted
        ? undefined
        : providerRetryDelaysMs[attempt - 1];
      await onFailure?.({
        ...failure,
        attempt,
        outcome: exhausted ? 'exhausted' : 'retrying',
        ...(nextDelayMs === undefined ? {} : { nextDelayMs }),
      });
      if (exhausted) {
        throw new ProviderCallError({ ...failure, attempts: attempt });
      }

      await retryDelay(nextDelayMs);
      attempt += 1;
    }
  }
};

export const providerFailure = (error) => {
  if (error instanceof ProviderCallError) return error.failure;
  if (!(error instanceof ProviderErrorObject)) return null;
  return { ...failureDetails(error), attempts: 1 };
};
