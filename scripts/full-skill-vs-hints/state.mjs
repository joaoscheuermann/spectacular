import { randomUUID } from 'node:crypto';
import { mkdir, readFile, rename, unlink, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';

import * as z from 'zod';

const choiceSchema = z.enum(['a', 'b', 'both', 'neither']);
const winnerSchema = z.enum(['full skill', 'hints', 'both', 'neither']);
const caseSchema = z
  .object({
    name: z.string().min(1),
    objective: z.string().min(1),
    source: z.string().min(1).optional(),
  })
  .strict();
const configSchema = z
  .object({
    mode: z.enum(['human', 'judge']),
    model: z.string().min(1),
    judgeModel: z.string().min(1),
    temperature: z.number().finite(),
    rounds: z.number().int().positive().max(Number.MAX_SAFE_INTEGER),
    cases: z.array(caseSchema).min(1),
  })
  .strict();
const resultSchema = z
  .object({
    name: z.string().min(1),
    round: z.number().int().positive(),
    choice: choiceSchema,
    rationale: z.string(),
    winner: winnerSchema,
    goals: z.array(z.string()),
    optionA: z.array(z.string()),
    optionB: z.array(z.string()),
    fullSkill: z.enum(['a', 'b']),
    skills: z.array(z.string()),
  })
  .strict();
const pendingSchema = z
  .object({
    caseName: z.string().min(1),
    round: z.number().int().positive(),
    goals: z.array(z.string()),
    optionA: z.array(z.string()),
    optionB: z.array(z.string()),
    fullSkill: z.enum(['a', 'b']),
    skills: z.array(z.string()),
    choice: choiceSchema.optional(),
  })
  .strict();
const failureSchema = z
  .object({
    caseName: z.string().min(1),
    round: z.number().int().positive(),
    at: z.string().min(1),
    code: z.string().min(1),
  })
  .strict();
const runSchema = z
  .object({
    id: z.string().uuid(),
    status: z.enum(['running', 'paused', 'completed']),
    createdAt: z.string().min(1),
    updatedAt: z.string().min(1),
    config: configSchema,
    results: z.array(resultSchema),
    pending: pendingSchema.nullable(),
    failures: z.array(failureSchema),
  })
  .strict();

const timestamp = () => new Date().toISOString();

const invalidRun = (cause) =>
  new Error('Invalid llm-lab run checkpoint.', { cause });

const keyOf = (name, round) => `${name}\u0000${round}`;

const firstUnfinishedRound = (run) => {
  const completed = new Set(
    run.results.map(({ name, round }) => keyOf(name, round)),
  );
  for (const current of run.config.cases) {
    for (let round = 1; round <= run.config.rounds; round += 1) {
      if (!completed.has(keyOf(current.name, round))) {
        return { caseName: current.name, round };
      }
    }
  }

  return null;
};

const validateInvariants = (run) => {
  const caseNames = new Set(run.config.cases.map(({ name }) => name));
  if (caseNames.size !== run.config.cases.length) {
    throw invalidRun(new Error('Duplicate case names.'));
  }

  const completed = new Set();
  for (const result of run.results) {
    const key = keyOf(result.name, result.round);
    if (
      !caseNames.has(result.name) ||
      result.round > run.config.rounds ||
      completed.has(key)
    ) {
      throw invalidRun(new Error('Invalid completed round.'));
    }
    completed.add(key);
  }

  if (run.pending !== null) {
    const key = keyOf(run.pending.caseName, run.pending.round);
    if (
      !caseNames.has(run.pending.caseName) ||
      run.pending.round > run.config.rounds ||
      completed.has(key)
    ) {
      throw invalidRun(new Error('Invalid pending round.'));
    }
  }

  for (const failure of run.failures) {
    if (!caseNames.has(failure.caseName) || failure.round > run.config.rounds) {
      throw invalidRun(new Error('Invalid failed round.'));
    }
  }

  const next = firstUnfinishedRound(run);
  if (
    run.pending !== null &&
    (next === null ||
      keyOf(next.caseName, next.round) !==
        keyOf(run.pending.caseName, run.pending.round))
  ) {
    throw invalidRun(new Error('Pending round is not the next round.'));
  }
  if (run.status === 'completed' && next !== null) {
    throw invalidRun(new Error('Completed run has unfinished rounds.'));
  }

  return run;
};

const parseRun = (value) => {
  try {
    return validateInvariants(runSchema.parse(value));
  } catch (error) {
    if (
      error instanceof Error &&
      error.message === 'Invalid llm-lab run checkpoint.'
    ) {
      throw error;
    }
    throw invalidRun(error);
  }
};

const updated = (run, patch, now = timestamp()) =>
  parseRun({ ...run, ...patch, updatedAt: now });

export const createRun = (config, options = {}) => {
  const now = options.now ?? timestamp();

  return parseRun({
    id: options.id ?? randomUUID(),
    status: 'running',
    createdAt: now,
    updatedAt: now,
    config,
    results: [],
    pending: null,
    failures: [],
  });
};

export const nextRound = (run) => {
  if (run.pending !== null) {
    return { caseName: run.pending.caseName, round: run.pending.round };
  }
  return firstUnfinishedRound(run);
};

export const beginRound = (run, pending, now) => {
  const next = nextRound(run);
  if (
    run.pending !== null ||
    next === null ||
    keyOf(next.caseName, next.round) !== keyOf(pending.caseName, pending.round)
  ) {
    throw new Error('Cannot checkpoint an unexpected llm-lab round.');
  }

  return updated(run, { status: 'running', pending }, now);
};

export const recordChoice = (run, choice, now) => {
  if (run.pending === null) {
    throw new Error('Cannot record a choice without a pending llm-lab round.');
  }

  return updated(
    run,
    { pending: { ...run.pending, choice: choiceSchema.parse(choice) } },
    now,
  );
};

const finishRound = (run, evaluation, now) => {
  const { caseName, round, goals, optionA, optionB, fullSkill, skills } =
    run.pending;
  const { choice, rationale } = evaluation;
  const winner =
    choice === 'both' || choice === 'neither'
      ? choice
      : choice === fullSkill
        ? 'full skill'
        : 'hints';

  return updated(
    run,
    {
      results: [
        ...run.results,
        {
          name: caseName,
          round,
          choice,
          rationale: rationale.trim(),
          winner,
          goals,
          optionA,
          optionB,
          fullSkill,
          skills,
        },
      ],
      pending: null,
    },
    now,
  );
};

export const completeRound = (run, rationale, now) => {
  if (run.pending?.choice === undefined) {
    throw new Error('Cannot complete an unanswered llm-lab round.');
  }
  return finishRound(run, { choice: run.pending.choice, rationale }, now);
};

export const completeJudgment = (run, judgment, now) => {
  if (run.pending === null) {
    throw new Error('Cannot judge without a pending llm-lab round.');
  }
  const evaluation = z
    .object({
      choice: choiceSchema,
      rationale: z.string(),
    })
    .strict()
    .parse(judgment);
  return finishRound(run, evaluation, now);
};

export const recordFailure = (run, task, code, now = timestamp()) =>
  updated(
    run,
    {
      failures: [
        ...run.failures,
        { caseName: task.caseName, round: task.round, at: now, code },
      ],
    },
    now,
  );

export const setRunStatus = (run, status, now) => updated(run, { status }, now);

export const assertCompatibleRun = (run, expected) => {
  const actual = {
    mode: run.config.mode,
    model: run.config.model,
    judgeModel: run.config.judgeModel,
    temperature: run.config.temperature,
    cases: run.config.cases,
  };
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(
      'The checkpoint does not match the current llm-lab config.',
    );
  }
};

export const saveRun = async (path, value) => {
  let run;
  try {
    run = parseRun(value);
  } catch (error) {
    throw invalidRun(error);
  }

  await mkdir(dirname(path), { recursive: true });
  const temporary = `${path}.${process.pid}.${randomUUID()}.tmp`;

  try {
    await writeFile(temporary, `${JSON.stringify(run, null, 2)}\n`, {
      encoding: 'utf8',
      flag: 'wx',
      mode: 0o600,
    });
    await rename(temporary, path);
  } catch (error) {
    await unlink(temporary).catch(() => undefined);
    throw error;
  }
};

export const loadRun = async (path) => {
  try {
    return parseRun(JSON.parse(await readFile(path, 'utf8')));
  } catch (error) {
    throw invalidRun(error);
  }
};
