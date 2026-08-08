import {
  RunSpecV1,
  type Case,
  type Condition,
  type ExecutionRecord,
  type RunSpec,
} from '../schemas/index.js';
import { contentHash } from '../core/hash.js';
import { createPrng } from '../core/prng.js';
import { PRIMARY_MODEL, REPETITIONS } from '../config/index.js';

export interface ScheduleInput {
  readonly studyId: string;
  readonly cases: readonly Case[];
  readonly conditions: readonly Condition[];
  readonly seed: string;
  readonly repetitions?: number;
  readonly capture?: 'structure' | 'io';
  readonly freezeHash?: string | null;
}

const specId = (value: unknown): string =>
  `run.${contentHash(value).slice(0, 32)}`;

/** Creates paired blocks with one shared seed and randomized condition order. */
export const createSchedule = (input: ScheduleInput): readonly RunSpec[] => {
  const repetitions = input.repetitions ?? REPETITIONS;
  if (!Number.isSafeInteger(repetitions) || repetitions <= 0) {
    throw new TypeError('repetitions must be a positive safe integer');
  }

  const blocks = input.cases.flatMap((benchmarkCase) =>
    Array.from({ length: repetitions }, (_, index) => ({
      benchmarkCase,
      repetition: index + 1,
      pairedBlock: `${benchmarkCase.id}.rep.${index + 1}`,
    })),
  );
  let order = 0;
  return blocks.flatMap((block) => {
    const rng = createPrng(input.seed).fork(block.pairedBlock);
    const seed = rng.integer(2_147_483_647);
    return rng.shuffle(input.conditions).map((condition) => {
      const identity = {
        studyId: input.studyId,
        caseId: block.benchmarkCase.id,
        conditionId: condition.id,
        repetition: block.repetition,
        seed,
      };
      const run = RunSpecV1.parse({
        schemaVersion: 1,
        id: specId(identity),
        studyId: input.studyId,
        phase: block.benchmarkCase.phase,
        caseId: block.benchmarkCase.id,
        conditionId: condition.id,
        repetition: block.repetition,
        seed,
        pairedBlock: block.pairedBlock,
        order,
        model: PRIMARY_MODEL,
        capture: input.capture ?? 'structure',
        freezeHash: input.freezeHash ?? null,
      });
      order += 1;
      return run;
    });
  });
};

export type ResumeAction = 'run' | 'retry-technical' | 'skip-terminal';

/** Allows one retry only when every prior attempt failed before model activity. */
export const resumeAction = (
  attempts: readonly ExecutionRecord[],
): ResumeAction => {
  if (attempts.length === 0) return 'run';
  const terminal = attempts.some(
    (record) =>
      record.status !== 'infrastructure' || record.firstModelCallStarted,
  );
  if (terminal || attempts.length >= 2) return 'skip-terminal';
  const retriable = attempts.every(
    (record) => record.infrastructureFailure?.beforeFirstModelCall === true,
  );
  return retriable ? 'retry-technical' : 'skip-terminal';
};

export interface ResumePlan {
  readonly run: RunSpec;
  readonly action: ResumeAction;
}

export const planResume = async (
  schedule: readonly RunSpec[],
  readAttempts: (runId: string) => Promise<readonly ExecutionRecord[]>,
): Promise<readonly ResumePlan[]> =>
  Promise.all(
    schedule.map(async (run) => ({
      run,
      action: resumeAction(await readAttempts(run.id)),
    })),
  );

/** Builds failure-only diagnostic runs and links each oracle to its failed parent. */
export const createOracleSchedule = (
  input: Omit<ScheduleInput, 'conditions' | 'repetitions'> & {
    readonly oracles: readonly Condition[];
  },
  failedParents: readonly ExecutionRecord[],
): readonly RunSpec[] => {
  const byCase = new Map(input.cases.map((entry) => [entry.id, entry]));
  return failedParents
    .filter((record) => record.status === 'failed')
    .flatMap((parent) => {
      const benchmarkCase = byCase.get(parent.run.caseId);
      if (benchmarkCase === undefined) return [];
      return createSchedule({
        ...input,
        cases: [benchmarkCase],
        conditions: input.oracles,
        repetitions: 1,
      }).map((run) =>
        RunSpecV1.parse({
          ...run,
          id: specId({
            oracleParentRunId: parent.run.id,
            conditionId: run.conditionId,
          }),
          pairedBlock: `oracle.${parent.run.id}`,
          seed: createPrng(input.seed)
            .fork(parent.run.id)
            .integer(2_147_483_647),
          oracleParentRunId: parent.run.id,
        }),
      );
    })
    .map((run, order) => RunSpecV1.parse({ ...run, order }));
};
