/** Generates a vector for text. */
export type Embedding = (data: string) => Promise<ReadonlyArray<number>>;

/** Configuration for an in-memory vector database. */
export type VectorDatabaseOptions = {
  readonly dimensions: number;
  readonly embedding: Embedding;
};

/** One cosine-similarity match returned by a search. */
export type VectorSearchResult<Data = unknown> = {
  readonly data: Data;
  readonly score: number;
};

/** In-memory vector storage and cosine-similarity search operations. */
export type VectorDatabase<Data = unknown> = {
  add(data: Data, transform: (data: Data) => string): Promise<void>;
  search(
    query: string,
    topK: number,
  ): Promise<ReadonlyArray<VectorSearchResult<Data>>>;
};
