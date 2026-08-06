import express, { type Router } from 'express';

import type { RunningVm } from '../lib/vms.js';

export type CreateVmsRouterOptions = {
  readonly list: () => readonly RunningVm[];
};

/** Creates the mountable routes for observing active VM runtimes. */
export const createVmsRouter = ({ list }: CreateVmsRouterOptions): Router => {
  const router = express.Router();

  router.get('/', (_request, response) => {
    response.json(list());
  });

  return router;
};
