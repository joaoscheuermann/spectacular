import type { LlmProvider } from 'llms';
import type { Logger } from 'pino';
import type { VectorDatabase } from 'victor';
import type { Skill } from 'bundle';

import goals from './goals.js';
import candidates from './candidates.js';
import hints from './hints.js';
import revision from './revision.js';

const HINT_LIMIT = 3;

export interface DecomposeContext {
  logger: Logger;
  provider: LlmProvider;
  vectors: VectorDatabase<Skill>;
  model: string;
}

export async function decompose(
  prompt: string,
  { provider, logger, vectors, model }: DecomposeContext,
) {
  logger.debug({ msg: 'starting decomposition', step: 'P0' });

  // Initial decomposition
  const p0Graph = await goals(model, prompt, { provider });
  const p0Skills = await candidates(prompt, p0Graph, HINT_LIMIT, { vectors });
  const p0Hints = await hints(model, p0Graph, p0Skills, {
    provider,
    logger,
    vectors,
  });

  // Revision
  const p1Graph = await revision(model, prompt, p0Graph, p0Hints, {
    provider,
    logger,
  });

  return p1Graph;
}
