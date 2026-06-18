import { withExploration } from './agents/exploration.js';
import { withIntent } from './agents/intent.js';
import { withOpenQuestions } from './agents/open-questions.js';
import { withProductRequirements } from './agents/product-requirements.js';
import { withRequestUnderstanding } from './agents/request-understanding.js';
import { withTechnicalRequirements } from './agents/technical-requirements.js';
import type { PromptArtifact, PromptWorkflowOptions } from './types/prompt.js';
import {
  normalizeArtifact,
  withoutRequirements,
} from './utils/artifacts.js';
import { renderPromptData } from './utils/render.js';

/** Creates the canonical in-memory PROMPT artifact from the user's first prompt. */
export const createPromptArtifact = (
  initialUserPrompt: string,
): PromptArtifact => ({
  name: 'PROMPT',
  mime: 'text/markdown',
  data: {
    initialUserPrompt,
  },
  template: renderPromptData,
});

/** Runs Step 01 prompt ingestion as a direct agent pipeline without writing PROMPT.md. */
export const prompt = async (
  artifact: PromptArtifact,
  options: PromptWorkflowOptions,
): Promise<PromptArtifact> => {
  const normalized = normalizeArtifact(artifact);
  const explored = await withExploration(normalized, options);
  const understood = await withRequestUnderstanding(explored, options);
  const intent = await withIntent(understood, options);
  const questioned = await withOpenQuestions(intent, options);

  if (questioned.hasBlockingQuestions) {
    return withoutRequirements(questioned.artifact);
  }

  const product = await withProductRequirements(questioned.artifact, options);

  return withTechnicalRequirements(product, options);
};
