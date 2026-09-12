import { createHash } from 'node:crypto';

import pino from 'pino';

import { SkillSchema } from 'bundle';
import type { LlmProvider } from 'llms';
import type { MosaicAgent, MosaicOptions, MosaicResult } from 'mosaic';
import {
  mosaic as evaluationMosaic,
  type MosaicEvaluationOptions,
} from 'mosaic/evaluation';
import type { Tool } from 'tool';

export const compositionArms = [
  'no-skills',
  'fixed-top-k',
  'mosaic',
  'oracle',
] as const;
/** Pinned cross-encoder used for body-aware candidate reranking. */
export const defaultCompositionRerankerModel = 'cohere/rerank-v3.5';

export type CompositionArm = (typeof compositionArms)[number];

export interface CompositionCase {
  readonly id: string;
  readonly dataset: string;
  readonly request: string;
  readonly goldSkillIds: readonly string[];
}

export interface CompositionSkill {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly body: string;
}

export interface RankedCompositionSkill {
  readonly skillId: string;
  readonly score: number;
}

export interface CompositionProfile {
  readonly provider: LlmProvider;
  readonly model: string;
  readonly rerankerProvider?: LlmProvider;
  readonly rerankerModel?: string;
}

export interface CompositionRunInput {
  readonly arm: CompositionArm;
  readonly benchmarkCase: CompositionCase;
  readonly skills: readonly CompositionSkill[];
  readonly ranking: readonly RankedCompositionSkill[];
  readonly profile: CompositionProfile;
  readonly tools?: readonly Tool[];
  readonly topK?: number;
  readonly maxHintCandidates?: number;
  readonly maxRetrievedCandidates?: number;
  readonly maxSkills?: number;
  readonly maxTurns?: number;
}

export interface CompositionCaseResult {
  readonly id: string;
  readonly dataset: string;
  readonly arm: CompositionArm;
  readonly status: MosaicResult['status'];
  readonly rawOutput: string;
  readonly skillIdsUsed: readonly string[];
  readonly candidateSkillIds: readonly string[];
}

type CreateWorkflow = (
  options: MosaicOptions,
  evaluation?: MosaicEvaluationOptions,
) => MosaicAgent;

export interface CompositionRunDependencies {
  readonly createWorkflow?: CreateWorkflow;
}

type Prepared = {
  readonly workflow: MosaicAgent;
  readonly canonicalToId: ReadonlyMap<string, string>;
  readonly requiredIds: readonly string[];
};

/** Creates one arm over a frozen retrieval ranking and shared model profile. */
export const createCompositionWorkflow = (
  input: CompositionRunInput,
  createWorkflow: CreateWorkflow = evaluationMosaic,
): MosaicAgent => prepare(input, createWorkflow).workflow;

/** Runs one case and returns an SRA-compatible model-output projection. */
export const runCompositionCase = async (
  input: CompositionRunInput,
  dependencies: CompositionRunDependencies = {},
): Promise<CompositionCaseResult> => {
  const prepared = prepare(
    input,
    dependencies.createWorkflow ?? evaluationMosaic,
  );

  const result = await prepared.workflow.prompt(input.benchmarkCase.request, {
    capture: 'structure',
  });

  const selected = unique(
    result.nodes.flatMap(({ bundle }) => bundle?.skills ?? []),
  );

  const candidates = unique(
    result.nodes.flatMap(({ candidates: values }) =>
      values.map(({ skillName }) => skillName),
    ),
  );

  const toIds = (names: readonly string[]): readonly string[] =>
    names.flatMap((name) => {
      const id = prepared.canonicalToId.get(name);

      return id === undefined ? [] : [id];
    });

  return {
    id: input.benchmarkCase.id,
    dataset: input.benchmarkCase.dataset,
    arm: input.arm,
    status: result.status,
    rawOutput: result.status === 'completed' ? result.delivery.markdown : '',
    skillIdsUsed:
      input.arm === 'mosaic' ? toIds(selected) : prepared.requiredIds,
    candidateSkillIds: toIds(candidates),
  };
};

const prepare = (
  input: CompositionRunInput,
  createWorkflow: CreateWorkflow,
): Prepared => {
  validateInput(input);

  const canonical = canonicalSkills(input.skills);
  const byId = new Map(canonical.map((entry) => [entry.id, entry]));

  const ranked = input.ranking.map(({ skillId, score }) => ({
    ...required(byId, skillId),
    score,
  }));

  const gold =
    input.arm === 'oracle'
      ? input.benchmarkCase.goldSkillIds.map((id) => required(byId, id))
      : [];
  const topK = input.topK ?? 3;
  const retrieved = input.maxRetrievedCandidates ?? 50;
  const shortlist = ranked.slice(0, retrieved);

  const requiredEntries =
    input.arm === 'fixed-top-k'
      ? ranked.slice(0, topK)
      : input.arm === 'oracle'
        ? gold
        : [];
  const menuEntries = input.arm === 'mosaic' ? shortlist : requiredEntries;
  const menu = menuEntries.map(({ skill }) => skill);
  const requiredSkills = requiredEntries.map(({ skill }) => skill);
  const tools = input.tools ?? [];
  const maxSkills = input.arm === 'mosaic' ? (input.maxSkills ?? 6) : 0;

  const options: MosaicOptions = {
    logger: pino({ enabled: false }),
    providers: {
      planning: input.profile.provider,
      revision: input.profile.provider,
      execution: input.profile.provider,
      reranker: input.profile.rerankerProvider ?? input.profile.provider,
    },
    models: {
      planning: { model: input.profile.model, effort: 'low' },
      revision: { model: input.profile.model, effort: 'low' },
      execution: { model: input.profile.model, effort: 'low' },
      reranker: input.profile.rerankerModel ?? defaultCompositionRerankerModel,
      embedder: input.profile.model,
    },
    routing: {
      maxHintCandidates: input.maxHintCandidates ?? 6,
      maxRetrievedCandidates: retrieved,
      maxSkills,
    },
    execution: { maxTurns: input.maxTurns ?? 16 },
    revision: { max: 0 },
    skills: { required: requiredSkills, menu, retriever: noSearch },
    tools: { required: tools, menu: tools, retriever: noSearch },
  };

  const evaluation: MosaicEvaluationOptions = {
    hooks: {
      ...(input.arm === 'no-skills'
        ? { feedbackPlan: async () => 'unchanged' as const }
        : {}),
      ...(input.arm === 'mosaic'
        ? {
            retrieval: async ({ limit }: { readonly limit: number }) =>
              shortlist.slice(0, limit).map(({ skill, score }) => ({
                skill,
                score,
              })),
          }
        : {}),
    },
  };

  return {
    workflow: createWorkflow(options, evaluation),
    canonicalToId: new Map(canonical.map(({ id, skill }) => [skill.name, id])),
    requiredIds: requiredEntries.map(({ id }) => id),
  };
};

const canonicalSkills = (values: readonly CompositionSkill[]) => {
  const seen = new Set<string>();
  const names = new Set<string>();

  return values.map((value) => {
    if (seen.has(value.id)) {throw new Error(`Duplicate skill id: ${value.id}`);}

    seen.add(value.id);

    const name = `skill-${createHash('sha256').update(value.id).digest('hex').slice(0, 16)}`;

    if (names.has(name)) {throw new Error('Canonical skill name collision.');}

    names.add(name);

    return {
      id: value.id,
      skill: SkillSchema.parse({
        name,
        description: value.description || value.name,
        body: value.body,
        allowedTools: [],
      }),
    };
  });
};

const required = <Value>(
  values: ReadonlyMap<string, Value>,
  id: string,
): Value => {
  const value = values.get(id);

  if (value === undefined) {throw new Error(`Unknown skill id: ${id}`);}

  return value;
};

const validateInput = (input: CompositionRunInput): void => {
  const positive = (value: number, name: string): void => {
    if (!Number.isSafeInteger(value) || value <= 0)
      {throw new Error(`${name} must be a positive safe integer.`);}
  };

  positive(input.topK ?? 3, 'topK');

  positive(input.maxHintCandidates ?? 6, 'maxHintCandidates');

  positive(input.maxRetrievedCandidates ?? 50, 'maxRetrievedCandidates');

  positive(input.maxSkills ?? 6, 'maxSkills');

  positive(input.maxTurns ?? 16, 'maxTurns');

  if ((input.maxSkills ?? 6) > (input.maxRetrievedCandidates ?? 50)) {
    throw new Error('maxSkills must not exceed maxRetrievedCandidates.');
  }

  const ranked = new Set<string>();

  input.ranking.forEach(({ skillId, score }) => {
    if (!Number.isFinite(score))
      {throw new Error('Ranking score must be finite.');}

    if (ranked.has(skillId))
      {throw new Error(`Duplicate ranked skill: ${skillId}`);}

    ranked.add(skillId);
  });
};

const unique = (values: readonly string[]): readonly string[] => [
  ...new Set(values),
];

const noSearch = {
  search: async (): Promise<readonly never[]> => [],
};
