import { readFile } from 'node:fs/promises';

import {
  prepareSraPilot,
  scoreSraRetrievalFile,
  parseSraRetrievalResults,
  type PrepareSraPilotOptions,
  type SraPilotArtifactManifest,
} from './composition/sra-artifacts.js';
import { compositionArms, type CompositionArm } from './composition/runner.js';
import {
  parseSraCorpusJson,
  parseSraInstancesJson,
} from './composition/sra-fixtures.js';
import {
  runSraDataset,
  type SraRunOptions,
  type SraRunSummary,
} from './composition/sra-run.js';
import type { SraAggregateRetrievalMetrics } from './composition/sra-metrics.js';
import {
  scoreSraOutputFile,
  sraSkillProjections,
  type SraOutputSkillMetrics,
  type SraSkillProjection,
} from './composition/sra-output-metrics.js';
import { createProvider, type ProviderProfile } from './run.js';
import {
  runPlanningBenchmark,
  type PlanningBenchmarkRunOptions,
  type PlanningBenchmarkRunSummary,
  type PlanningRunProgress,
} from './composition/planning-run.js';
import type { PlanningModelEvent } from './composition/planning-model.js';
import {
  planningBenchmarkManifest,
  type PlanningBenchmarkManifest,
} from './composition/planning-artifacts.js';
import {
  prepareSkillsbenchComposition,
  writeSkillsbenchPreparation,
  type PrepareSkillsbenchCompositionOptions,
  type SkillsbenchPreparation,
} from './composition/skillsbench-prepare.js';

type Read = (path: string, encoding: 'utf8') => Promise<string>;

export interface CompositionCommandDependencies {
  readonly readFile?: Read;
  readonly prepareSra?: (
    options: PrepareSraPilotOptions,
  ) => Promise<SraPilotArtifactManifest>;
  readonly scoreSra?: (
    inputPath: string,
    k: number,
    outputPath?: string,
  ) => Promise<SraAggregateRetrievalMetrics>;
  readonly scoreSraOutput?: (
    inputPath: string,
    goldPath: string,
    projection: SraSkillProjection,
    k: number,
    outputPath?: string,
  ) => Promise<SraOutputSkillMetrics>;
  readonly runSra?: (options: SraRunOptions) => Promise<SraRunSummary>;
  readonly createProfile?: () => ProviderProfile;
  readonly planningManifest?: () => PlanningBenchmarkManifest;
  readonly runPlanning?: (
    options: PlanningBenchmarkRunOptions,
  ) => Promise<PlanningBenchmarkRunSummary>;
  readonly progress?: PlanningBenchmarkRunOptions['progress'];
  readonly prepareSkillsbench?: (
    options: PrepareSkillsbenchCompositionOptions,
  ) => Promise<SkillsbenchPreparation>;
  readonly writeSkillsbench?: (
    outputPath: string,
    preparation: SkillsbenchPreparation,
  ) => Promise<void>;
}

export interface SkillsbenchPreparationSummary {
  readonly benchmark: 'SkillsBench Composition';
  readonly outputPath: string;
  readonly tasks: number;
  readonly catalogSkills: number;
  readonly catalogSha256: string;
  readonly rankingSha256: string;
  readonly arms: readonly string[];
}

type CommandResult =
  | SraPilotArtifactManifest
  | SraAggregateRetrievalMetrics
  | SraOutputSkillMetrics
  | SraRunSummary
  | PlanningBenchmarkManifest
  | PlanningBenchmarkRunSummary
  | SkillsbenchPreparationSummary;

/** Executes local composition preparation, scoring, and approved model runs. */
export const runCompositionCommand = async (
  values: readonly string[],
  dependencies: CompositionCommandDependencies = {},
): Promise<CommandResult> => {
  const args = [...values];
  const benchmark = args.shift();
  const action = args.shift();
  if (benchmark === 'planning') return planning(action, args, dependencies);
  if (benchmark === 'skillsbench')
    return skillsbench(action, args, dependencies);
  if (benchmark !== 'sra')
    throw new Error(
      'composition requires benchmark: sra, planning, or skillsbench',
    );
  if (action === 'prepare') return prepare(args, dependencies);
  if (action === 'score') return score(args, dependencies);
  if (action === 'score-output') return scoreOutput(args, dependencies);
  if (action === 'run') return run(args, dependencies);
  throw new Error(
    'composition sra requires action: prepare, score, score-output, or run',
  );
};

const skillsbench = async (
  action: string | undefined,
  args: string[],
  dependencies: CompositionCommandDependencies,
): Promise<SkillsbenchPreparationSummary> => {
  if (action !== 'prepare')
    throw new Error('composition skillsbench requires action: prepare');
  const sourceRoot = takeRequired(args, '--source');
  const outputPath = takeRequired(args, '--output');
  rejectArgs(args);
  const preparation = await (
    dependencies.prepareSkillsbench ?? prepareSkillsbenchComposition
  )({ sourceRoot });
  await (dependencies.writeSkillsbench ?? writeSkillsbenchPreparation)(
    outputPath,
    preparation,
  );
  return {
    benchmark: 'SkillsBench Composition',
    outputPath,
    tasks: preparation.catalogManifest.counts.tasks,
    catalogSkills: preparation.catalogManifest.counts.catalogSkills,
    catalogSha256: preparation.catalogManifest.catalogSha256,
    rankingSha256: preparation.contract.fixedRanking.sha256,
    arms: preparation.contract.arms.map(({ id }) => id),
  };
};

const planning = async (
  action: string | undefined,
  args: string[],
  dependencies: CompositionCommandDependencies,
): Promise<PlanningBenchmarkManifest | PlanningBenchmarkRunSummary> => {
  if (action === 'manifest') {
    rejectArgs(args);
    return (dependencies.planningManifest ?? planningBenchmarkManifest)();
  }
  if (action !== 'run')
    throw new Error('composition planning requires action: manifest or run');
  const approved = takeBoolean(args, '--yes-paid-run');
  if (!approved)
    throw new Error('Paid composition runs require --yes-paid-run.');
  const outputDir = takeRequired(args, '--output');
  const caseIds = takeAll(args, '--case');
  const maxTurns = optionalPositive(args, '--max-turns');
  rejectArgs(args);
  const profile =
    dependencies.createProfile?.() ??
    createProvider({ environment: { ...process.env } });
  return (dependencies.runPlanning ?? runPlanningBenchmark)({
    profile,
    outputDir,
    ...(caseIds.length === 0 ? {} : { caseIds }),
    ...(maxTurns === undefined ? {} : { maxTurns }),
    ...(dependencies.progress === undefined
      ? {}
      : { progress: dependencies.progress }),
  });
};

/** Formats one safe controlled-planning progress event for stderr. */
export const formatPlanningProgress = (event: PlanningRunProgress): string => {
  const prefix = '[mosaic-planning]';
  switch (event.type) {
    case 'run.started':
      return `${prefix} run started cases=${event.caseCount}`;
    case 'case.started':
      return `${prefix} case ${event.caseIndex}/${event.caseCount} started id=${event.caseId}`;
    case 'cache.hit':
      return `${prefix} cache hit case=${event.caseId} condition=${conditionName(event.condition)} operation=${event.operation}`;
    case 'model.call':
      return `${prefix} model call case=${event.caseId} condition=${conditionName(event.condition)} operation=${event.operation} call=${event.call}`;
    case 'structured.attempt':
      return `${prefix} structured attempt case=${event.caseId} condition=${conditionName(event.condition)} operation=${event.operation} attempt=${event.attempt} accepted=${yesNo(event.runtimeAccepted)} repair=${yesNo(event.feedbackSent)}`;
    case 'model.completed':
      return `${prefix} model completed case=${event.caseId} condition=${conditionName(event.condition)} operation=${event.operation} calls=${event.calls}`;
    case 'retrieval.completed':
      return `${prefix} retrieval completed case=${event.caseId} condition=${event.condition} matches=${event.matches.length}`;
    case 'case.completed':
      return `${prefix} case ${event.caseIndex}/${event.caseCount} completed id=${event.caseId} conditions=${event.conditionCount} model_calls=${event.modelCallCount}`;
    case 'run.completed':
      return `${prefix} run completed cases=${event.caseCount} model_calls=${event.modelCallCount}`;
  }
};

const conditionName = (condition: PlanningModelEvent['condition']): string =>
  condition ?? 'shared-p0';

const yesNo = (value: boolean): 'yes' | 'no' => (value ? 'yes' : 'no');

const prepare = async (
  args: string[],
  dependencies: CompositionCommandDependencies,
): Promise<SraPilotArtifactManifest> => {
  const sourceRoot = takeRequired(args, '--source');
  const outputDir = takeRequired(args, '--output');
  rejectArgs(args);
  return (dependencies.prepareSra ?? prepareSraPilot)({
    sourceRoot,
    outputDir,
  });
};

const score = async (
  args: string[],
  dependencies: CompositionCommandDependencies,
): Promise<SraAggregateRetrievalMetrics> => {
  const input = takeRequired(args, '--input');
  const k = positive(takeRequired(args, '--k'), '--k');
  const output = take(args, '--output');
  rejectArgs(args);
  return (dependencies.scoreSra ?? scoreSraRetrievalFile)(input, k, output);
};

const scoreOutput = async (
  args: string[],
  dependencies: CompositionCommandDependencies,
): Promise<SraOutputSkillMetrics> => {
  const input = takeRequired(args, '--input');
  const gold = takeRequired(args, '--gold');
  const projection = parseProjection(takeRequired(args, '--projection'));
  const k = positive(takeRequired(args, '--k'), '--k');
  const output = take(args, '--output');
  rejectArgs(args);
  return (dependencies.scoreSraOutput ?? scoreSraOutputFile)(
    input,
    gold,
    projection,
    k,
    output,
  );
};

const run = async (
  args: string[],
  dependencies: CompositionCommandDependencies,
): Promise<SraRunSummary> => {
  const approved = takeBoolean(args, '--yes-paid-run');
  if (!approved)
    throw new Error('Paid composition runs require --yes-paid-run.');
  const arm = parseArm(takeRequired(args, '--arm'));
  const instancesPath = takeRequired(args, '--instances');
  const corpusPath = takeRequired(args, '--corpus');
  const retrievalPath = take(args, '--retrieval');
  const outputPath = takeRequired(args, '--output');
  const topK = optionalPositive(args, '--top-k');
  const maxHintCandidates = optionalPositive(args, '--max-hint-candidates');
  const maxRetrievedCandidates = optionalPositive(
    args,
    '--max-retrieved-candidates',
  );
  const maxSkills = optionalPositive(args, '--max-skills');
  const maxTurns = optionalPositive(args, '--max-turns');
  const rerankerModel = take(args, '--reranker-model');
  rejectArgs(args);
  if (
    (arm === 'fixed-top-k' || arm === 'mosaic') &&
    retrievalPath === undefined
  ) {
    throw new Error(`${arm} requires --retrieval.`);
  }
  const read = dependencies.readFile ?? readFile;
  const [instancesSource, corpusSource, retrievalSource] = await Promise.all([
    read(instancesPath, 'utf8'),
    read(corpusPath, 'utf8'),
    retrievalPath === undefined
      ? Promise.resolve(undefined)
      : read(retrievalPath, 'utf8'),
  ]);
  const baseProfile =
    dependencies.createProfile?.() ??
    createProvider({ environment: { ...process.env } });
  const profile = {
    ...baseProfile,
    ...(rerankerModel === undefined ? {} : { rerankerModel }),
  };
  return (dependencies.runSra ?? runSraDataset)({
    arm,
    instances: parseSraInstancesJson(instancesSource),
    corpus: parseSraCorpusJson(corpusSource),
    retrieval:
      retrievalSource === undefined
        ? []
        : parseSraRetrievalResults(retrievalSource),
    profile,
    outputPath,
    ...(topK === undefined ? {} : { topK }),
    ...(maxHintCandidates === undefined ? {} : { maxHintCandidates }),
    ...(maxRetrievedCandidates === undefined ? {} : { maxRetrievedCandidates }),
    ...(maxSkills === undefined ? {} : { maxSkills }),
    ...(maxTurns === undefined ? {} : { maxTurns }),
  });
};

const parseArm = (value: string): CompositionArm => {
  const arm = compositionArms.find((candidate) => candidate === value);
  if (arm === undefined) throw new Error(`Unknown composition arm: ${value}`);
  return arm;
};

const parseProjection = (value: string): SraSkillProjection => {
  const projection = sraSkillProjections.find(
    (candidate) => candidate === value,
  );
  if (projection === undefined)
    throw new Error(`Unknown SRA skill projection: ${value}`);
  return projection;
};

const optionalPositive = (args: string[], flag: string): number | undefined => {
  const value = take(args, flag);
  return value === undefined ? undefined : positive(value, flag);
};

const positive = (value: string, flag: string): number => {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0)
    throw new Error(`${flag} requires a positive safe integer.`);
  return parsed;
};

const takeBoolean = (args: string[], flag: string): boolean => {
  const index = args.indexOf(flag);
  if (index < 0) return false;
  args.splice(index, 1);
  return true;
};

const takeRequired = (args: string[], flag: string): string => {
  const value = take(args, flag);
  if (value === undefined) throw new Error(`${flag} requires a value.`);
  return value;
};

const take = (args: string[], flag: string): string | undefined => {
  const index = args.indexOf(flag);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (value === undefined || value.startsWith('--'))
    throw new Error(`${flag} requires a value.`);
  args.splice(index, 2);
  return value;
};

const takeAll = (args: string[], flag: string): readonly string[] => {
  const values: string[] = [];
  for (;;) {
    const value = take(args, flag);
    if (value === undefined) return values;
    values.push(value);
  }
};

const rejectArgs = (args: readonly string[]): void => {
  if (args.length > 0) throw new Error(`Unknown argument: ${args[0]}`);
};

export const compositionUsage = [
  '  npx nx run mosaic-benchmark:run -- composition planning manifest',
  '  npx nx run mosaic-benchmark:run -- composition planning run --output <dir> [--case <id>] [--max-turns <N>] --yes-paid-run',
  '  npx nx run mosaic-benchmark:run -- composition skillsbench prepare --source <pinned-checkout> --output <artifact.json>',
  '  npx nx run mosaic-benchmark:run -- composition sra prepare --source <data/bench> --output <dir>',
  '  npx nx run mosaic-benchmark:run -- composition sra score --input <retrieval.json> --k <K> [--output <metrics.json>]',
  '  npx nx run mosaic-benchmark:run -- composition sra score-output --input <inference.jsonl> --gold <gold.json> --projection <selected|candidates> --k <K> [--output <metrics.json>]',
  '  npx nx run mosaic-benchmark:run -- composition sra run --arm <no-skills|fixed-top-k|mosaic|oracle> --instances <file> --corpus <file> [--retrieval <file>] --output <file> [--reranker-model <model>] --yes-paid-run',
].join('\n');
