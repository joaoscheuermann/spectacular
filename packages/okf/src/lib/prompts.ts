import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export type Prompts = {
  readonly classify: string;
  readonly frontmatter: string;
};

const STAGES = ['classify', 'frontmatter'] as const;
const TARGET = /^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/u;
const DEVICE = /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/u;

/** Loads the stage-level prompts that do not depend on classification. */
export const loadPrompts = async (target = 'default'): Promise<Prompts> => {
  const model = validatePromptTarget(target);

  const entries = await Promise.all(
    STAGES.map(
      async (stage) =>
        [
          stage,
          await readPrompt([stage, model], `${stage} prompt target ${model}`),
        ] as const,
    ),
  );

  return Object.fromEntries(entries) as Prompts;
};

export const validatePromptTarget = (target: string): string => {
  if (!TARGET.test(target) || DEVICE.test(target) || target === 'scenarios') {
    throw new Error(`Invalid OKF prompt target: ${target}`);
  }

  return target;
};

/** Reads a non-empty system prompt from the nearest package prompt tree. */
export const readPrompt = async (
  location: readonly string[],
  label: string,
): Promise<string> => {
  const start = path.dirname(fileURLToPath(import.meta.url));
  const file = await findPrompt(start, location);

  if (file === undefined) {
    throw new Error(`Cannot load OKF ${label}: prompt file does not exist`);
  }

  try {
    const prompt = (await fs.readFile(file, 'utf-8')).trim();
    if (prompt.length === 0) throw new Error('prompt is empty');
    return prompt;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Cannot load OKF ${label}: ${message}`);
  }
};

const findPrompt = async (
  start: string,
  location: readonly string[],
): Promise<string | undefined> => {
  let current = start;

  while (true) {
    const file = path.join(current, 'prompts', ...location, 'SYSTEM_PROMPT.md');
    try {
      if ((await fs.stat(file)).isFile()) return file;
    } catch (error) {
      if (!isMissing(error)) throw error;
    }

    const parent = path.dirname(current);
    if (parent === current) return undefined;
    current = parent;
  }
};

const isMissing = (error: unknown): error is NodeJS.ErrnoException =>
  error instanceof Error &&
  'code' in error &&
  (error.code === 'ENOENT' || error.code === 'ENOTDIR');
