import { createFetchTransport, createUnifiedProvider } from 'llms';
import pino from 'pino';
import pretty from 'pino-pretty';

const wait = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

const validateRetry = ({ attempts, delayMs, backoffMultiplier }) => {
  if (!Number.isSafeInteger(attempts) || attempts < 1) {
    throw new TypeError('Action attempts must be a positive safe integer.');
  }
  if (!Number.isSafeInteger(delayMs) || delayMs < 0) {
    throw new TypeError('Action delay must be a non-negative safe integer.');
  }
  if (!Number.isSafeInteger(backoffMultiplier) || backoffMultiplier < 1) {
    throw new TypeError(
      'Action backoff multiplier must be a positive safe integer.',
    );
  }
};

const createLogger = (logPath) =>
  pino(
    { level: 'info' },
    pino.multistream([
      {
        stream: pretty({
          colorize: process.stdout.isTTY,
          destination: process.stdout,
          sync: true,
        }),
      },
      {
        stream: pino.destination({
          dest: logPath,
          mkdir: true,
          sync: true,
        }),
      },
    ]),
  );

/** Owns provider construction, retries, logging, and usage for one run. */
export const createRuntime = (output) => {
  const logger = createLogger(output.logPath);
  const recordUsage = (operation, model, usage) =>
    logger.info(
      output.recordProviderUsage({ operation, model, usage }),
      'Provider usage recorded',
    );
  const recordAttempt = (operation, model) =>
    logger.info(
      output.recordProviderAttempt({ operation, model }),
      'Provider attempt started',
    );
  const action = async (message, callback, retry) => {
    validateRetry(retry);
    logger.info(message);
    let attempt = 1;
    while (true) {
      try {
        return await callback();
      } catch (error) {
        if (attempt === retry.attempts) throw error;
        const nextDelayMs =
          retry.delayMs * retry.backoffMultiplier ** (attempt - 1);
        logger.warn(
          { action: message, attempt, attempts: retry.attempts, nextDelayMs },
          'Action failed; retrying',
        );
        await wait(nextDelayMs);
        attempt += 1;
      }
    }
  };
  const createProvider = () => {
    const apiKey = process.env.OPENROUTER_API_KEY?.trim();
    if (!apiKey) throw new Error('OPENROUTER_API_KEY is required.');
    return createUnifiedProvider({
      transport: createFetchTransport(),
      apiKey,
      logger,
    });
  };

  return { logger, action, createProvider, recordAttempt, recordUsage };
};
