import type { StructuredOutputSchema } from 'llms';
import type { z } from 'zod';

import type { Completion } from '../src/completion.js';
import type { EvolutionConfig } from '../src/schema.js';

type Text = (system: string, input: string) => string | Promise<string>;
type Structured = (system: string, input: string) => unknown | Promise<unknown>;

export const fakeCompletion = (
  text: Text = async () => '',
  structured: Structured = async () => ({
    passed: true,
    ambiguous: false,
    rationale: 'Clear.',
  }),
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
    {
      id: 'optimizer',
      type: 'openrouter',
      tokenEnv: 'OPENROUTER_API_KEY',
    },
  ],
  models: [
    {
      id: 'lfm2.5-8b-a1b',
      provider: 'local',
      model: 'lfm2.5-8b-a1b',
    },
  ],
  optimizer: { provider: 'optimizer', model: 'optimizer-model' },
  judges: [
    { provider: 'local', model: 'judge-one' },
    { provider: 'local', model: 'judge-two' },
  ],
  evolution: {
    targetAccuracy: 1,
    plateauPatience: 2,
    maxEpochs: 2,
  },
});
