import {
  CaseV1,
  ConditionV1,
  type ExecutionRecord,
  type RunSpec,
} from '../schemas/index.js';
import { SKILLS, validateCatalog } from '../catalog/index.js';
import {
  CONDITIONS,
  PRIMARY_CONDITIONS,
  validateAblationMatrix,
} from '../conditions/index.js';
import { TOOLS } from '../runtime/index.js';
import {
  DORIC_SMOKE_CASES,
  DORIC_SMOKE_CONDITIONS,
  PILOT_CASES,
  calibrateModels,
  createSchedule,
  packageStudy,
  planResume,
  runConformance,
  validateCases,
  writeFreeze,
} from '../study/index.js';

export interface ValidationIssue {
  readonly surface: string;
  readonly code: string;
  readonly detail: string;
}

export interface ValidationReport {
  readonly valid: boolean;
  readonly issues: readonly ValidationIssue[];
  readonly counts: {
    readonly skills: number;
    readonly tools: number;
    readonly pilotCases: number;
    readonly conditions: number;
    readonly smokes: number;
  };
}

/** Implements the provider-free `validate` command. */
export const validateInstrument = (): ValidationReport => {
  const issues: ValidationIssue[] = [
    ...validateCatalog(SKILLS).map((issue) => ({
      surface: 'catalog',
      code: issue.code,
      detail: `${issue.path}: ${issue.message}`,
    })),
    ...validateCases(PILOT_CASES).map((issue) => ({
      surface: 'cases',
      code: issue.code,
      detail: `${issue.caseId}: ${issue.detail}`,
    })),
    ...validateAblationMatrix().map((issue) => ({
      surface: 'conditions',
      code: 'ablation_matrix',
      detail: issue.conditionId,
    })),
  ];
  PILOT_CASES.filter((entry) => !CaseV1.safeParse(entry).success).forEach(
    (entry) => {
      issues.push({ surface: 'cases', code: 'schema', detail: entry.id });
    },
  );
  CONDITIONS.filter((entry) => !ConditionV1.safeParse(entry).success).forEach(
    (entry) => {
      issues.push({ surface: 'conditions', code: 'schema', detail: entry.id });
    },
  );
  if (TOOLS.length !== 24)
    issues.push({
      surface: 'tools',
      code: 'count',
      detail: String(TOOLS.length),
    });
  if (DORIC_SMOKE_CASES.length !== 6 || DORIC_SMOKE_CONDITIONS.length !== 6) {
    issues.push({
      surface: 'smoke',
      code: 'count',
      detail: 'expected six smoke pairs',
    });
  }
  return {
    valid: issues.length === 0,
    issues,
    counts: {
      skills: SKILLS.length,
      tools: TOOLS.length,
      pilotCases: PILOT_CASES.length,
      conditions: CONDITIONS.length,
      smokes: DORIC_SMOKE_CASES.length,
    },
  };
};

/** Implements the provider-free `pilot` command schedule (60 x 6 x 5). */
export const preparePilot = (
  studyId: string,
  seed: string,
): readonly RunSpec[] => {
  const schedule = createSchedule({
    studyId,
    cases: PILOT_CASES,
    conditions: PRIMARY_CONDITIONS,
    seed,
    repetitions: 5,
  });
  if (schedule.length !== 1_800)
    throw new Error('pilot schedule must contain exactly 1,800 runs');
  return schedule;
};

export interface ScheduledRunResult {
  readonly run: RunSpec;
  readonly disposition: 'executed' | 'retried' | 'skipped';
  readonly record?: ExecutionRecord;
}

/** Implements `run --resume` without embedding a provider or paid model call. */
export const runSchedule = async (
  schedule: readonly RunSpec[],
  options: {
    readonly resume: boolean;
    readonly readAttempts: (
      runId: string,
    ) => Promise<readonly ExecutionRecord[]>;
    readonly execute: (run: RunSpec) => Promise<ExecutionRecord>;
  },
): Promise<readonly ScheduledRunResult[]> => {
  const plan = await planResume(schedule, options.readAttempts);
  if (!options.resume && plan.some((entry) => entry.action !== 'run')) {
    throw new TypeError('existing attempts require --resume');
  }
  const results: ScheduledRunResult[] = [];
  for (const entry of [...plan].sort(
    (left, right) => left.run.order - right.run.order,
  )) {
    if (entry.action === 'skip-terminal') {
      results.push({ run: entry.run, disposition: 'skipped' });
      continue;
    }
    const record = await options.execute(entry.run);
    results.push({
      run: entry.run,
      disposition: entry.action === 'retry-technical' ? 'retried' : 'executed',
      record,
    });
  }
  return results;
};

/** Dependency-neutral operations consumed by the CLI entrypoint. */
export const HARNESS_COMMANDS = {
  validate: validateInstrument,
  conformance: runConformance,
  calibrateModels,
  pilot: preparePilot,
  freeze: writeFreeze,
  run: runSchedule,
  package: packageStudy,
} as const;
