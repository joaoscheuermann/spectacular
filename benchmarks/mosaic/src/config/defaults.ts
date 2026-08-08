export const PRIMARY_MODEL = {
  provider: 'openai',
  model: 'openai/gpt-5.6-luna',
  effort: 'medium',
} as const;

export const RERANKER_MODEL = 'voyageai/rerank-2.5-lite';
export const EMBEDDING_MODEL = {
  model: 'voyageai/voyage-4-large',
  dimensions: 2048,
} as const;

export const FIRST_REPLICATION_CANDIDATE = {
  provider: 'openrouter',
  model: 'qwen/qwen3.7-flash',
  effort: 'medium',
} as const;

export const REPETITIONS = 5;
export const PILOT_FAMILIES = 60;
