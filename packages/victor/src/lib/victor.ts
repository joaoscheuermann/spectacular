import type {
  Embedding,
  VectorDatabase,
  VectorDatabaseOptions,
  VectorSearchResult,
} from './types/vector-database.js';

type Entry<Data> = {
  readonly data: Data;
  readonly vector: ReadonlyArray<number>;
  readonly magnitude: Magnitude;
};

type Magnitude = {
  readonly scale: number;
  readonly norm: number;
};

type ScoredEntry<Data> = VectorSearchResult<Data> & {
  readonly index: number;
};

const invalid = (message: string): Error =>
  new TypeError(`Invalid vector database ${message}.`);

const magnitudeOf = (vector: ReadonlyArray<number>): Magnitude | undefined => {
  let scale = 0;
  let sum = 0;

  for (const value of vector) {
    const absolute = Math.abs(value);

    if (absolute === 0) {
      continue;
    }

    if (scale < absolute) {
      sum = 1 + sum * (scale / absolute) ** 2;
      scale = absolute;
      continue;
    }

    sum += (absolute / scale) ** 2;
  }

  return scale === 0 ? undefined : { scale, norm: Math.sqrt(sum) };
};

const validateDimensions = (dimensions: number): void => {
  if (!Number.isSafeInteger(dimensions) || dimensions <= 0) {
    throw invalid('dimensions: expected a positive safe integer');
  }
};

function validateEmbedding(embedding: unknown): asserts embedding is Embedding {
  if (typeof embedding !== 'function') {
    throw invalid('embedding: expected a function');
  }
}

const validateVector = (
  value: unknown,
  dimensions: number,
): ReadonlyArray<number> => {
  if (!Array.isArray(value) || value.length !== dimensions) {
    throw invalid(
      `embedding result: expected an array with ${dimensions} dimensions`,
    );
  }

  if (
    !value.every((entry) => typeof entry === 'number' && Number.isFinite(entry))
  ) {
    throw invalid('embedding result: expected only finite numbers');
  }

  const vector = [...value];
  const magnitude = magnitudeOf(vector);

  if (magnitude === undefined) {
    throw invalid('embedding result: expected a non-zero magnitude');
  }

  return vector;
};

const cosine = (
  left: ReadonlyArray<number>,
  leftMagnitude: Magnitude,
  right: ReadonlyArray<number>,
  rightMagnitude: Magnitude,
): number => {
  const score = left.reduce(
    (sum, value, index) =>
      sum +
      (value / leftMagnitude.scale / leftMagnitude.norm) *
        (right[index] / rightMagnitude.scale / rightMagnitude.norm),
    0,
  );

  return Math.max(-1, Math.min(1, score));
};

/** Creates an in-memory database that embeds stored values and queries. */
export const createVectorDatabase = <Data = unknown>(
  options: VectorDatabaseOptions,
): VectorDatabase<Data> => {
  const dimensions = options?.dimensions;
  const embedding = options?.embedding;

  validateDimensions(dimensions);
  validateEmbedding(embedding);

  const entries: Entry<Data>[] = [];

  const embed = async (data: string): Promise<Entry<Data>['vector']> =>
    validateVector(await embedding(data), dimensions);

  return {
    async add(data: Data, transform: (data: Data) => string): Promise<void> {
      if (typeof transform !== 'function') {
        throw invalid('transform: expected a function');
      }

      const text = transform(data);

      if (typeof text !== 'string') {
        throw invalid('transform result: expected a string');
      }

      const vector = await embed(text);
      const magnitude = magnitudeOf(vector);

      if (magnitude === undefined) {
        throw invalid('embedding result: expected a non-zero magnitude');
      }

      entries.push({ data, vector, magnitude });
    },

    async search(
      query,
      topK,
    ): Promise<ReadonlyArray<VectorSearchResult<Data>>> {
      if (!Number.isSafeInteger(topK) || topK < 0) {
        throw invalid('topK: expected a nonnegative safe integer');
      }

      if (topK === 0 || entries.length === 0) {
        return [];
      }

      const vector = await embed(query);
      const magnitude = magnitudeOf(vector);

      if (magnitude === undefined) {
        throw invalid('embedding result: expected a non-zero magnitude');
      }

      return entries
        .map(
          (entry, index): ScoredEntry<Data> => ({
            data: entry.data,
            score: cosine(entry.vector, entry.magnitude, vector, magnitude),
            index,
          }),
        )
        .sort(
          (left, right) => right.score - left.score || left.index - right.index,
        )
        .slice(0, topK)
        .map(({ data, score }) => ({ data, score }));
    },
  };
};
