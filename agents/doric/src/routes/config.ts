import { Router } from 'express';

import { ConfigInputSchema } from '../lib/config.js';
import type { ConfigService } from '../lib/config-service.js';
import { sendError } from '../lib/http.js';

/** Exposes credential-free singleton Mosaic configuration replacement. */
export const createConfigRouter = (service: ConfigService): Router => {
  const router = Router();

  router.get('/', (_request, response) =>
    response.json(service.current().snapshot),
  );
  router.put('/', async (request, response) => {
    const parsed = ConfigInputSchema.safeParse(request.body);
    if (!parsed.success) {
      sendError(
        response,
        422,
        'invalid_config',
        'The Mosaic configuration is invalid.',
      );
      return;
    }
    try {
      response.json(await service.replace(parsed.data));
    } catch {
      sendError(
        response,
        503,
        'configuration_rejected',
        'The Mosaic configuration could not be activated.',
      );
    }
  });

  return router;
};
