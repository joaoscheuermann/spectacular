import type { StructuredOutputSchema } from 'llms';
import type { z } from 'zod';

import type { Completion } from '../src/completion.js';
import type { EvolutionConfig, Scenario } from '../src/schema.js';

type Text = (system: string, input: string) => string | Promise<string>;
type Structured = (system: string, input: string) => unknown | Promise<unknown>;

export const fakeCompletion = (
  text: Text = async () => '',
  structured: Structured = async () => ({ results: [] }),
): Completion => ({
  async text(system, input) {
    return text(system, input);
  },
  async structured<Schema extends StructuredOutputSchema>(
    system: string,
    input: string,
    schema: Schema,
  ): Promise<z.output<Schema>> {
    return schema.parse(await structured(system, input));
  },
});

export const validConfig = (): EvolutionConfig => ({
  providers: [
    { id: 'local', type: 'lmstudio-openai' },
    { id: 'optimizer', type: 'openrouter', tokenEnv: 'OPENROUTER_API_KEY' },
  ],
  models: [{ id: 'model', provider: 'local', model: 'target-model' }],
  optimizer: { provider: 'optimizer', model: 'optimizer-model' },
  judge: { provider: 'local', model: 'judge-model' },
  evals: [{ id: 'global', assertion: 'Always applies.' }],
  evolution: {
    accuracy: 1,
    patience: { epochs: 2 },
    epochs: 3,
    history: { limit: 30 },
  },
});

export const scenarios = (): readonly Scenario[] => [
  {
    id: 'train-case',
    split: 'train',
    input: 'training input',
    evals: [{ id: 'global', assertion: 'Always applies.' }],
  },
  {
    id: 'validation-case',
    split: 'validation',
    input: 'validation input',
    evals: [{ id: 'global', assertion: 'Always applies.' }],
  },
];

export const matrix = (
  evalIds: readonly string[],
  passed = true,
): readonly {
  readonly evalId: string;
  readonly sampleIndex: number;
  readonly reasoning: string;
  readonly passed: boolean;
}[] =>
  evalIds.flatMap((evalId) =>
    [0, 1, 2].map((sampleIndex) => ({
      evalId,
      sampleIndex,
      reasoning: 'Private evidence.',
      passed,
    })),
  );
