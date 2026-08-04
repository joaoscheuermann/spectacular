import type { Skill } from 'bundle';
import type { LlmProvider } from 'llms';
import type { Logger } from 'pino';
import type { Tool } from 'tool';
import type { VectorDatabase } from 'victor';

export interface MosaicOptions {
  readonly logger: Logger;
  readonly provider: LlmProvider;
  readonly models: {
    readonly default: string;
    readonly reranker: string;
    readonly embedder: string;
  };
  readonly skills: {
    readonly required: readonly Skill[];
    readonly menu: readonly Skill[];
    readonly embeddings: VectorDatabase<Skill>;
  };
  readonly tools: {
    readonly required: readonly Tool[];
    readonly menu: readonly Tool[];
    readonly embeddings: VectorDatabase<Tool>;
  };
}
