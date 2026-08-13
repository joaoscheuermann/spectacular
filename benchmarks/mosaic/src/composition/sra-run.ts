import { createHash } from 'node:crypto';
import { appendFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';

import {
  defaultCompositionRerankerModel,
  runCompositionCase,
  type CompositionArm,
  type CompositionCaseResult,
  type CompositionProfile,
  type CompositionRunInput,
  type CompositionSkill,
} from './runner.js';
import type { SraCorpusSkill, SraInstance } from './sra-fixtures.js';
import type { SraRetrievalRecord } from './sra-metrics.js';

export interface SraRunOptions {
  readonly arm: CompositionArm;
  readonly instances: readonly SraInstance[];
  readonly corpus: readonly SraCorpusSkill[];
  readonly retrieval: readonly SraRetrievalRecord[];
  readonly profile: CompositionProfile;
  readonly outputPath: string;
  readonly topK?: number;
  readonly maxHintCandidates?: number;
  readonly maxRetrievedCandidates?: number;
  readonly maxSkills?: number;
  readonly maxTurns?: number;
}

export interface SraRunSummary {
  readonly arm: CompositionArm;
  readonly dataset: string;
  readonly outputPath: string;
  readonly completed: number;
  readonly skipped: number;
  readonly failed: number;
  readonly runSha256: string;
  readonly manifestPath: string;
}

export interface SraRunManifest {
  readonly schemaVersion: 1;
  readonly benchmark: 'SRA-Bench';
  readonly arm: CompositionArm;
  readonly dataset: string;
  readonly model: string;
  readonly rerankerModel: string | null;
  readonly controls: {
    readonly topK: number;
    readonly maxHintCandidates: number;
    readonly maxRetrievedCandidates: number;
    readonly maxSkills: number;
    readonly maxTurns: number;
  };
  readonly inputs: {
    readonly instancesSha256: string;
    readonly instanceCount: number;
    readonly corpusSha256: string;
    readonly corpusSkillCount: number;
    readonly retrievalSha256: string | null;
  };
  readonly runSha256: string;
}

type RunCase = (input: CompositionRunInput) => Promise<CompositionCaseResult>;

export interface SraRunDependencies {
  readonly runCase?: RunCase;
}

/** Runs one homogeneous SRA dataset sequentially with append-only resume. */
export const runSraDataset = async (
  options: SraRunOptions,
  dependencies: SraRunDependencies = {},
): Promise<SraRunSummary> => {
  const dataset = oneDataset(options.instances);
  const corpus = new Map(
    options.corpus.map((skill) => [skill.skill_id, skill]),
  );
  if (corpus.size !== options.corpus.length)
    throw new Error('SRA corpus contains duplicate skill ids.');
  const retrieval = retrievalMap(options.retrieval);
  validateRun(options, corpus, retrieval);
  await mkdir(dirname(options.outputPath), { recursive: true });
  const manifest = runManifest(options, dataset);
  const completedIds = await existingIds(
    options.outputPath,
    new Set(options.instances.map(({ instance_id: id }) => id)),
    manifest,
  );
  const manifestPath = `${options.outputPath}.run.json`;
  await requireRunIdentity(manifestPath, manifest, completedIds.size > 0);
  const pending = options.instances.filter(
    ({ instance_id: id }) => !completedIds.has(id),
  );
  let completed = 0;
  let failed = 0;

  for (const instance of pending) {
    const input = compositionInput(options, instance, corpus, retrieval);
    const result = await (dependencies.runCase ?? runCompositionCase)(input);
    await appendFile(
      options.outputPath,
      `${JSON.stringify(inferenceRecord(result, options.profile))}\n`,
      'utf8',
    );
    completed += 1;
    if (result.status !== 'completed') failed += 1;
  }

  return {
    arm: options.arm,
    dataset,
    outputPath: options.outputPath,
    completed,
    skipped: completedIds.size,
    failed,
    runSha256: manifest.runSha256,
    manifestPath,
  };
};

const compositionInput = (
  options: SraRunOptions,
  instance: SraInstance,
  corpus: ReadonlyMap<string, SraCorpusSkill>,
  retrieval: ReadonlyMap<string, SraRetrievalRecord>,
): CompositionRunInput => {
  const record = retrieval.get(instance.instance_id);
  if (
    (options.arm === 'fixed-top-k' || options.arm === 'mosaic') &&
    record === undefined
  ) {
    throw new Error(`Missing retrieval for ${instance.instance_id}.`);
  }
  if (record !== undefined) assertGold(instance, record);
  const ranked = record?.retrieved ?? [];
  const ids =
    options.arm === 'oracle'
      ? instance.skill_annotations
      : options.arm === 'no-skills'
        ? []
        : ranked.map(({ skill_id }) => skill_id);

  return {
    arm: options.arm,
    benchmarkCase: {
      id: instance.instance_id,
      dataset: instance.dataset,
      request: instance.question,
      goldSkillIds: instance.skill_annotations,
    },
    skills: unique(ids).map((id) => compositionSkill(required(corpus, id))),
    ranking: ranked.map(({ skill_id: skillId, score }, index) => ({
      skillId,
      score: score ?? ranked.length - index,
    })),
    profile: options.profile,
    ...(options.topK === undefined ? {} : { topK: options.topK }),
    ...(options.maxHintCandidates === undefined
      ? {}
      : { maxHintCandidates: options.maxHintCandidates }),
    ...(options.maxRetrievedCandidates === undefined
      ? {}
      : { maxRetrievedCandidates: options.maxRetrievedCandidates }),
    ...(options.maxSkills === undefined
      ? {}
      : { maxSkills: options.maxSkills }),
    ...(options.maxTurns === undefined ? {} : { maxTurns: options.maxTurns }),
  };
};

const compositionSkill = (skill: SraCorpusSkill): CompositionSkill => ({
  id: skill.skill_id,
  name: skill.name || skill.skill_id,
  description: skill.description || skill.name || 'Skill instructions.',
  body: skill.content,
});

const inferenceRecord = (
  result: CompositionCaseResult,
  profile: Pick<CompositionProfile, 'model'>,
) => ({
  instance_id: result.id,
  dataset: result.dataset,
  method: method(result.arm),
  model: profile.model,
  raw_output: result.rawOutput,
  ...(result.skillIdsUsed.length === 0
    ? {}
    : { skill_ids_used: result.skillIdsUsed }),
  meta: {
    workflow_status: result.status,
    candidate_skill_ids: result.candidateSkillIds,
  },
  ...(result.status === 'completed'
    ? {}
    : { error: `mosaic_${result.status}` }),
});

const retrievalMap = (
  values: readonly SraRetrievalRecord[],
): ReadonlyMap<string, SraRetrievalRecord> => {
  const result = new Map<string, SraRetrievalRecord>();
  values.forEach((record) => {
    if (result.has(record.instance_id))
      throw new Error(`Duplicate retrieval: ${record.instance_id}`);
    result.set(record.instance_id, record);
  });
  return result;
};

const assertGold = (
  instance: SraInstance,
  record: SraRetrievalRecord,
): void => {
  const annotated = [...instance.skill_annotations].sort();
  const recorded = [...record.gold_skill_ids].sort();
  if (JSON.stringify(annotated) !== JSON.stringify(recorded))
    throw new Error(
      `Retrieval gold skills differ for ${instance.instance_id}.`,
    );
};

const existingIds = async (
  path: string,
  expected: ReadonlySet<string>,
  manifest: SraRunManifest,
): Promise<ReadonlySet<string>> => {
  let source: string;
  try {
    source = await readFile(path, 'utf8');
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return new Set();
    throw error;
  }
  const ids = source
    .split('\n')
    .filter((line) => line.trim().length > 0)
    .map((line) => {
      const value = JSON.parse(line) as Readonly<Record<string, unknown>>;
      if (typeof value.instance_id !== 'string')
        throw new Error('Existing SRA output has no instance_id.');
      if (
        value.dataset !== manifest.dataset ||
        value.method !== method(manifest.arm) ||
        value.model !== manifest.model ||
        typeof value.raw_output !== 'string'
      ) {
        throw new Error('Existing SRA output record has a different identity.');
      }
      validateStringArray(value.skill_ids_used, 'skill_ids_used');
      if (!isRecord(value.meta))
        throw new Error('Existing SRA output record has invalid meta.');
      validateStringArray(
        value.meta.candidate_skill_ids,
        'candidate_skill_ids',
        true,
      );
      return value.instance_id;
    });
  if (new Set(ids).size !== ids.length)
    throw new Error('Existing SRA output contains duplicate instances.');
  if (ids.some((id) => !expected.has(id)))
    throw new Error('Existing SRA output contains an unexpected instance.');
  return new Set(ids);
};

const runManifest = (
  options: SraRunOptions,
  dataset: string,
): SraRunManifest => {
  const controls = {
    topK: options.topK ?? 3,
    maxHintCandidates: options.maxHintCandidates ?? 6,
    maxRetrievedCandidates: options.maxRetrievedCandidates ?? 50,
    maxSkills: options.maxSkills ?? 6,
    maxTurns: options.maxTurns ?? 16,
  };
  const inputs = {
    instancesSha256: digest('sra-instances-v1', options.instances),
    instanceCount: options.instances.length,
    corpusSha256: digestSequence(
      'sra-corpus-v1',
      [...options.corpus].sort((left, right) =>
        left.skill_id.localeCompare(right.skill_id),
      ),
    ),
    corpusSkillCount: options.corpus.length,
    retrievalSha256:
      options.retrieval.length === 0
        ? null
        : digest(
            'sra-retrieval-v1',
            [...options.retrieval].sort((left, right) =>
              left.instance_id.localeCompare(right.instance_id),
            ),
          ),
  };
  const unsigned = {
    schemaVersion: 1 as const,
    benchmark: 'SRA-Bench' as const,
    arm: options.arm,
    dataset,
    model: options.profile.model,
    rerankerModel:
      options.arm === 'mosaic'
        ? (options.profile.rerankerModel ?? defaultCompositionRerankerModel)
        : null,
    controls,
    inputs,
  };
  return {
    ...unsigned,
    runSha256: digest('sra-run-v1', unsigned),
  };
};

const requireRunIdentity = async (
  path: string,
  expected: SraRunManifest,
  hasOutput: boolean,
): Promise<void> => {
  let source: string;
  try {
    source = await readFile(path, 'utf8');
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    if (hasOutput)
      throw new Error('Existing SRA output has no run identity manifest.');
    await writeFile(path, `${JSON.stringify(expected, null, 2)}\n`, {
      encoding: 'utf8',
      flag: 'wx',
    });
    return;
  }
  let actual: unknown;
  try {
    actual = JSON.parse(source) as unknown;
  } catch {
    throw new Error('Existing SRA run identity manifest is invalid.');
  }
  if (canonicalJson(actual) !== canonicalJson(expected))
    throw new Error('Existing SRA output belongs to a different run identity.');
};

const digest = (domain: string, value: unknown): string =>
  createHash('sha256')
    .update(`${domain}\n${canonicalJson(value)}`)
    .digest('hex');

const digestSequence = (domain: string, values: readonly unknown[]): string => {
  const hash = createHash('sha256').update(`${domain}\n`);
  values.forEach((value) => hash.update(`${canonicalJson(value)}\n`));
  return hash.digest('hex');
};

const canonicalJson = (value: unknown): string => {
  if (value === null) return 'null';
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (typeof value === 'object') {
    const record = value as Readonly<Record<string, unknown>>;
    return `{${Object.keys(record)
      .filter((key) => record[key] !== undefined)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`)
      .join(',')}}`;
  }
  const serialized = JSON.stringify(value);
  if (serialized === undefined) throw new TypeError('Value is not JSON data.');
  return serialized;
};

const oneDataset = (instances: readonly SraInstance[]): string => {
  if (instances.length === 0) throw new Error('SRA run requires instances.');
  const datasets = new Set(instances.map(({ dataset }) => dataset));
  if (datasets.size !== 1) throw new Error('SRA run requires one dataset.');
  return instances[0]!.dataset;
};

const method = (arm: CompositionArm): string =>
  `mosaic_${arm.replaceAll('-', '_')}`;

const validateStringArray = (
  value: unknown,
  label: string,
  required = false,
): void => {
  if (value === undefined && !required) return;
  if (
    !Array.isArray(value) ||
    value.some((child) => typeof child !== 'string' || child.trim() === '') ||
    new Set(value).size !== value.length
  ) {
    throw new Error(`Existing SRA output record has invalid ${label}.`);
  }
};

const isRecord = (value: unknown): value is Readonly<Record<string, unknown>> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const validateRun = (
  options: SraRunOptions,
  corpus: ReadonlyMap<string, SraCorpusSkill>,
  retrieval: ReadonlyMap<string, SraRetrievalRecord>,
): void => {
  const instanceIds = options.instances.map(({ instance_id: id }) => id);
  if (new Set(instanceIds).size !== instanceIds.length)
    throw new Error('SRA run contains duplicate instances.');
  const usesRanking = options.arm === 'fixed-top-k' || options.arm === 'mosaic';
  if (usesRanking && retrieval.size !== options.instances.length)
    throw new Error('SRA ranked arms require one retrieval per instance.');
  if (!usesRanking && retrieval.size > 0)
    throw new Error('SRA no-skills and oracle arms do not accept retrieval.');
  const knownInstances = new Set(instanceIds);
  if ([...retrieval.keys()].some((id) => !knownInstances.has(id)))
    throw new Error('SRA retrieval contains an unexpected instance.');
  options.instances.forEach((instance) => {
    if (
      new Set(instance.skill_annotations).size !==
      instance.skill_annotations.length
    )
      throw new Error(`Duplicate gold skill for ${instance.instance_id}.`);
    const record = retrieval.get(instance.instance_id);
    if (usesRanking && record === undefined)
      throw new Error(`Missing retrieval for ${instance.instance_id}.`);
    if (record !== undefined) assertGold(instance, record);
    if (
      record !== undefined &&
      new Set(record.retrieved.map(({ skill_id: id }) => id)).size !==
        record.retrieved.length
    )
      throw new Error(`Duplicate retrieved skill for ${instance.instance_id}.`);
    if (
      record?.retrieved.some(
        ({ score }) => score !== undefined && !Number.isFinite(score),
      )
    )
      throw new Error(`Invalid retrieval score for ${instance.instance_id}.`);
    const requiredIds = [
      ...instance.skill_annotations,
      ...(record?.retrieved.map(({ skill_id: id }) => id) ?? []),
    ];
    requiredIds.forEach((id) => required(corpus, id));
  });
  const controls = [
    [options.topK ?? 3, 'topK'],
    [options.maxHintCandidates ?? 6, 'maxHintCandidates'],
    [options.maxRetrievedCandidates ?? 50, 'maxRetrievedCandidates'],
    [options.maxSkills ?? 6, 'maxSkills'],
    [options.maxTurns ?? 16, 'maxTurns'],
  ] as const;
  controls.forEach(([value, label]) => {
    if (!Number.isSafeInteger(value) || value <= 0)
      throw new Error(`${label} must be a positive safe integer.`);
  });
  if ((options.maxSkills ?? 6) > (options.maxRetrievedCandidates ?? 50))
    throw new Error('maxSkills must not exceed maxRetrievedCandidates.');
};

const required = <Value>(
  values: ReadonlyMap<string, Value>,
  id: string,
): Value => {
  const value = values.get(id);
  if (value === undefined) throw new Error(`Missing corpus skill: ${id}`);
  return value;
};

const unique = (values: readonly string[]): readonly string[] => [
  ...new Set(values),
];
