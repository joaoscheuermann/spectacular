import type { Logger } from 'pino';

import type { Skill } from 'bundle';
import type { LlmProvider, ReasoningEffort } from 'llms';
import type { Tool } from 'tool';
import type { Search } from 'victor';

export interface MosaicOptions {
  readonly logger: Logger;
  readonly providers: {
    readonly planning: LlmProvider;
    readonly revision: LlmProvider;
    readonly execution: LlmProvider;
    readonly reranker: LlmProvider;
  };
  readonly models: {
    readonly planning: MosaicModelProfile;
    readonly revision: MosaicModelProfile;
    readonly execution: MosaicModelProfile;
    readonly reranker: string;
    readonly embedder: string;
  };
  readonly routing: {
    readonly maxHintCandidates: number;
    readonly maxRetrievedCandidates: number;
    readonly maxSkills: number;
  };
  readonly execution: {
    readonly maxTurns: number;
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

export interface MosaicModelProfile {
  readonly model: string;
  readonly effort: ReasoningEffort;
}
