import type { Skill } from 'bundle';
import type { LlmProvider } from 'llms';
import type { Logger } from 'pino';
import type { Tool } from 'tool';
import type { Search } from 'victor';

export interface MosaicOptions {
  readonly logger: Logger;
  readonly provider: LlmProvider;
  readonly models: {
    readonly default: string;
    readonly reranker: string;
  };
  readonly routing: {
    readonly maxCandidates: number;
    readonly maxSkills: number;
  };
  readonly revision: {
    readonly max: number;
  };
  readonly skills: {
    readonly required: readonly Skill[];
    readonly menu: readonly Skill[];
    readonly retriever: Search<Skill>;
  };
  readonly tools: {
    readonly required: readonly Tool[];
    readonly menu: readonly Tool[];
    readonly retriever: Search<Tool>;
  };
}
