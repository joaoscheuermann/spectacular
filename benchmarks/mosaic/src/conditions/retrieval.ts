import type { LlmProvider } from 'llms';

import { SKILLS, type MicroSkill } from '../catalog/index.js';

type RerankRequest = Parameters<LlmProvider['rerank']>[0];
type RerankResult = Awaited<ReturnType<LlmProvider['rerank']>>;

export interface SkillMatch {
  readonly skill: MicroSkill;
  readonly score: number;
}

/** Shared search and rerank boundary used by every skill-bearing condition. */
export interface SkillRetrieval {
  readonly models: {
    readonly embedder: string;
    readonly reranker: string;
  };
  readonly search: (
    query: string,
    topK: number,
  ) => Promise<readonly SkillMatch[]>;
  readonly rerank: (request: RerankRequest) => Promise<RerankResult>;
}

export interface SkillRetrievalSource {
  readonly models: SkillRetrieval['models'];
  readonly search: (
    query: string,
    topK: number,
  ) => Promise<
    ReadonlyArray<{
      readonly data: { readonly name: string };
      readonly score: number;
    }>
  >;
  readonly rerank: LlmProvider['rerank'];
}

const compare = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;

const canonicalMatches = (
  matches: Awaited<ReturnType<SkillRetrievalSource['search']>>,
  topK: number,
): readonly SkillMatch[] => {
  const catalog = new Map(SKILLS.map((skill) => [skill.id, skill]));
  const seen = new Set<string>();
  return matches
    .flatMap(({ data, score }) => {
      const skill = catalog.get(data.name);
      if (skill === undefined || seen.has(skill.id)) return [];
      if (!Number.isFinite(score))
        throw new TypeError('skill retrieval returned a non-finite score');
      seen.add(skill.id);
      return [{ skill, score }];
    })
    .slice(0, topK);
};

/** Adapts the frozen hybrid index and reranker into one injectable boundary. */
export const createSkillRetrieval = (
  source: SkillRetrievalSource,
): SkillRetrieval => ({
  models: source.models,
  search: async (query, topK) => {
    if (!Number.isSafeInteger(topK) || topK <= 0)
      throw new TypeError('skill retrieval topK must be positive');
    return canonicalMatches(await source.search(query, topK), topK);
  },
  rerank: (request) => source.rerank(request),
});

const fence = (value: string): string => {
  const longest = Math.max(
    0,
    ...(value.match(/`+/gu) ?? []).map((item) => item.length),
  );
  const marker = '`'.repeat(Math.max(3, longest + 1));
  const body = value.endsWith('\n') ? value : `${value}\n`;
  return `${marker}text\n${body}${marker}`;
};

const section = (heading: string, value: string): string =>
  `## ${heading}\n\n${fence(value)}`;

/** Stable human-readable document shared by baseline and MOSAIC reranking. */
export const skillDocument = (skill: MicroSkill): string =>
  [
    section('Canonical Skill Name', skill.id),
    section('Description', skill.description),
    section('Canonical Body', skill.body),
  ].join('\n\n');

export interface RankedSkill extends SkillMatch {
  readonly rank: number;
  readonly relevanceScore: number;
}

/** Validates and normalizes a complete reranker permutation. */
export const rankedSkills = (
  matches: readonly SkillMatch[],
  ranking: RerankResult,
): readonly RankedSkill[] => {
  if (ranking.length !== matches.length)
    throw new TypeError('skill reranker returned an incomplete ranking');
  const indices = new Set<number>();
  const ordered = ranking.map(({ index, relevanceScore }) => {
    if (
      !Number.isSafeInteger(index) ||
      index < 0 ||
      index >= matches.length ||
      indices.has(index) ||
      !Number.isFinite(relevanceScore)
    ) {
      throw new TypeError('skill reranker returned an invalid result');
    }
    indices.add(index);
    return { match: matches[index]!, relevanceScore };
  });
  return ordered
    .sort(
      (left, right) =>
        right.relevanceScore - left.relevanceScore ||
        compare(left.match.skill.id, right.match.skill.id),
    )
    .map(({ match, relevanceScore }, index) => ({
      ...match,
      relevanceScore,
      rank: index + 1,
    }));
};
