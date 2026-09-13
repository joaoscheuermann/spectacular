/*
Build the shared lexical and semantic skill indexes once for the run.
Receives the runtime and full catalog; returns hybrid search with bounded reranking.
Lexical documents contain names and bodies; embeddings and reranker documents use bodies.
This stage has no generative model prompt.
**/

import {
  createHybridSearch,
  createLexicalIndex,
  createVectorIndex,
} from 'victor';

export const createSearch = async ({ runtime, skills }) => {
  const { config, logger } = runtime;
  const provider = runtime.measured('retrieval');

  const embed = async (input) => {
    const response = await provider.embedding({
      model: config.embeddingModel,
      dimensions: config.embeddingDimensions,
      input,
      flags: { sensitiveOutput: true },
    });

    return response.embedding;
  };
  const lexical = createLexicalIndex({ logger });

  const semantic = createVectorIndex({
    logger,
    dimensions: config.embeddingDimensions,
    embedding: embed,
  });

  for (const skill of skills) {
    await lexical.add(skill, ({ name, body }) => name + '\n' + body);

    await semantic.add(skill, ({ body }) => body);
  }

  const hybrid = createHybridSearch({
    lexical,
    semantic,
    key: (skill) => skill.name,
    logger,
  });

  return async (query) => {
    const candidates = await hybrid.search(query, config.retrievalK);

    if (candidates.length === 0) {
      return [];
    }

    const response = await provider.rerank({
      model: config.rerankerModel,
      query,
      documents: candidates.map(({ data }) => data.body),
      topN: Math.min(config.topK, candidates.length),
      flags: { sensitiveOutput: true },
    });

    return response.results.map(({ index, relevanceScore }) => {
      const candidate = candidates[index];

      if (!candidate) {
        throw new Error('Unknown reranker index.');
      }

      return { ...candidate.data, score: relevanceScore };
    });
  };
};
