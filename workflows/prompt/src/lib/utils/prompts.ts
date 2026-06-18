import type { PromptData } from '../types/prompt.js';

export const artifactInput = (data: PromptData): string =>
  JSON.stringify(data, null, 2);
