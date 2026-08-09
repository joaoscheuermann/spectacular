import { join } from 'node:path';

import { cli, fail } from './study-runtime.mjs';

/** Builds or reuses the one paid, content-addressed retrieval index. */
export const setupIndex = async (context) => {
  const receipt = await cli({
    label: 'retrieval index setup',
    args: [
      'index',
      '--artifacts',
      context.config.root,
      '--prices',
      context.config.files.prices,
      '--candidate-model',
      context.config.candidate.model,
      '--yes-paid-probes',
      '--yes-paid-setup',
    ],
    receipt: join(context.paths.preflight, 'index.json'),
  });
  if (
    typeof receipt.result !== 'object' ||
    receipt.result === null ||
    typeof receipt.result.path !== 'string' ||
    typeof receipt.result.indexHash !== 'string'
  ) {
    fail('retrieval index setup returned no immutable artifact');
  }
  context.index = receipt.result;
};
