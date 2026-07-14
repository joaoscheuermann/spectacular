import type { StructuredOutputSchema } from 'llms';
import type { z } from 'zod';

import type { Completion } from '../src/completion.js';
import type { EvolutionConfig, Scenario } from '../src/schema.js';

type Text = (system: string, input: string) => string | Promise<string>;
type Structured = (system: string, input: string) => unknown | Promise<unknown>;

export type Deferred<Value> = {
  readonly promise: Promise<Value>;
  readonly resolve: (value: Value) => void;
  readonly reject: (reason?: unknown) => void;
};

export const deferred = <Value>(): Deferred<Value> => {
  let resolve!: (value: Value) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<Value>((accept, decline) => {
    resolve = accept;
    reject = decline;
  });
  return { promise, resolve, reject };
};

export const fakeCompletion = (
  text: Text = async () => '',
  structured: Structured = async () => ({
    reasoning: 'Private evidence.',
    passed: true,
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
    concurrency: { scenarios: 1, judgments: 1 },
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

export const judgment = (
  passed = true,
): {
  readonly reasoning: string;
  readonly passed: boolean;
} => ({
  reasoning: 'Private evidence.',
  passed,
});
