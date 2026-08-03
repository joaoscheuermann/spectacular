import { LlmProvider } from 'llms';
import { Logger } from 'pino';
import { VectorDatabase } from 'victor';
import { Skill } from '../types/skill.js';

import goals from './goals/index.js';
import candidates from './candidates/index.js';
import hints from './hints/index.js';
import revision from './revision/index.js';
import { K_HINT } from '../constants/index.js';

export interface DecomposeContext {
  logger: Logger;
  provider: LlmProvider;
  vectors: VectorDatabase<Skill>;
}

const model = `openai/gpt-5.6-luna`;

export async function decompose(
  prompt: string,
  { provider, logger, vectors }: DecomposeContext,
) {
  logger.info({ msg: 'starting decomposition', step: 'P0' });

  // Initial decomposition
  const p0Graph = await goals(model, prompt, { provider });
  const p0Skills = await candidates(prompt, p0Graph, K_HINT, { vectors });
  const p0Hints = await hints(model, p0Graph, p0Skills, {
    provider,
    logger,
    vectors,
  });

  // Revision
  const p1Graph = await revision(model, prompt, p0Graph, p0Hints, { provider, logger });

  return p1Graph;
}
