import type { LlmProvider, ReasoningEffort } from 'llms';

export type OkfConfig = {
  readonly provider: LlmProvider;
  readonly model: string;
  readonly effort?: ReasoningEffort;
  readonly promptTarget?: string;
  readonly batchSize?: number;
  readonly ignore?: readonly string[];
  readonly signal?: AbortSignal;
};

export type GenerateResult = {
  readonly root: string;
  readonly output: string;
  readonly index: string;
  readonly files: readonly string[];
  readonly generated: number;
  readonly cached: number;
};
