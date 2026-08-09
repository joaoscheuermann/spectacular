import { Router } from 'express';
import { z } from 'zod';

import { sendError } from '../lib/http.js';
import type { SessionService } from '../lib/session-service.js';

const createInput = z
  .object({
    prompt: z.string().refine((value) => value.trim().length > 0),
  })
  .strict();

const listInput = z.object({
  limit: z.coerce.number().int().safe().positive().max(100).default(50),
  cursor: z.uuid().optional(),
});

const idInput = z.object({ id: z.uuid() });

/** Exposes durable Mosaic session commands and cursor-based history. */
export const createSessionsRouter = (service: SessionService): Router => {
  const router = Router();

  router.post('/', async (request, response) => {
    const parsed = createInput.safeParse(request.body);
    if (!parsed.success) {
      sendError(
        response,
        422,
        'invalid_prompt',
        'A non-empty prompt is required.',
      );
      return;
    }
    response.status(202).json(await service.create(parsed.data.prompt));
  });

  router.get('/', async (request, response) => {
    const parsed = listInput.safeParse(request.query);
    if (!parsed.success) {
      sendError(response, 400, 'invalid_page', 'The session page is invalid.');
      return;
    }
    response.json(await service.list(parsed.data.limit, parsed.data.cursor));
  });

  router.post('/:id/terminate', async (request, response) => {
    const parsed = idInput.safeParse(request.params);
    if (!parsed.success) {
      sendError(
        response,
        400,
        'invalid_session_id',
        'The session ID is invalid.',
      );
      return;
    }
    const session = await service.terminate(parsed.data.id);
    if (session === undefined) {
      sendError(
        response,
        404,
        'session_not_found',
        'The session was not found.',
      );
      return;
    }
    response.json(session);
  });

  router.delete('/:id', async (request, response) => {
    const parsed = idInput.safeParse(request.params);
    if (!parsed.success) {
      sendError(
        response,
        400,
        'invalid_session_id',
        'The session ID is invalid.',
      );
      return;
    }
    const outcome = await service.delete(parsed.data.id);
    if (outcome === 'missing') {
      sendError(
        response,
        404,
        'session_not_found',
        'The session was not found.',
      );
      return;
    }
    if (outcome === 'active') {
      sendError(
        response,
        409,
        'session_active',
        'Active sessions cannot be deleted.',
      );
      return;
    }
    response.status(204).end();
  });

  return router;
};
