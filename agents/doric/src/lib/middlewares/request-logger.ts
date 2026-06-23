import type { RequestHandler } from 'express';

type RequestLogger = {
  readonly info: (
    bindings: {
      readonly method: string;
      readonly path: string;
      readonly statusCode: number;
      readonly durationMs: number;
    },
    message: string,
  ) => void;
};

/** Creates an Express middleware that logs completed HTTP requests. */
export const createRequestLogger = (logger: RequestLogger): RequestHandler => {
  return (request, response, next) => {
    const startedAt = process.hrtime.bigint();

    response.on('finish', () => {
      const durationMs = Number(process.hrtime.bigint() - startedAt) / 1_000_000;

      logger.info(
        {
          method: request.method,
          path: request.path,
          statusCode: response.statusCode,
          durationMs,
        },
        'HTTP request completed',
      );
    });

    next();
  };
};
