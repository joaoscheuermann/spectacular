import { isKind } from '../../src/lib/agents/classify/kinds.js';
import { readPrompt, validatePromptTarget } from '../../src/lib/prompts.js';

/** Selects the analysis system prompt for one prompt model and file kind. */
export const SYSTEM_PROMPT =
  (model: string) =>
  async (kind: string): Promise<string> => {
    const target = validatePromptTarget(model);
    if (!isKind(kind)) {
      throw new Error(`Unsupported OKF analysis kind: ${kind}`);
    }

    return readPrompt(
      ['analyze', kind, target],
      `analyze prompt for kind ${kind} target ${target}`,
    );
  };
