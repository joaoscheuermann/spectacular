#!/usr/bin/env node

import { relative, resolve } from 'node:path';
import { parseArgs } from 'node:util';

import {
  createFetchTransport,
  createUnifiedProvider,
  ProviderErrorObject,
} from 'llms';
import {
  cancel,
  intro,
  isCancel,
  log,
  note,
  outro,
  select,
  text,
} from '@clack/prompts';
import pino from 'pino';
import * as z from 'zod';

import { cases, loadSkills, runCases } from './cases.mjs';
import {
  assertCompatibleRun,
  beginRound,
  completeJudgment,
  completeRound,
  createRun,
  loadRun,
  nextRound,
  recordChoice,
  recordFailure,
  saveRun,
  setRunStatus,
} from './state.mjs';
import {
  extractHintsSystem,
  goalsSystem,
  goalsUser,
  judgeSystem,
  judgeUser,
  reviewHintsSystem,
  reviewSkillsSystem,
} from './prompts.mjs';

const logger = pino({
  enabled: true,
  transport: {
    target: 'pino-pretty',
  },
});

const model = 'deepseek/deepseek-v4-pro';
const judgeModel = 'google/gemini-3.7-flash';
const temperature = 0;
const checkpointDirectory = '.llm-lab/full-skill-vs-hints/runs';
const action = (str, callback) => {
  log.step(str);
  return callback();
};

const schemas = {
  goals: z.object({
    goals: z.array(z.string()).describe(''),
  }),
  hints: z.object({
    hints: z
      .array(
        z.object({
          effect: z.enum([
            'vocabulary',
            'gap',
            'division',
            'dependency',
            'execution',
          ]),
          evidence: z.string(),
        }),
      )
      .describe(''),
  }),
  judge: z.object({
    choice: z.enum(['a', 'b', 'both', 'neither']),
    rationale: z.string(),
  }),
};

const generateGoals = (provider, objective) =>
  action('Generating goals', async () => {
    const result = await provider.complete({
      model,
      temperature,
      messages: [
        { role: 'system', content: goalsSystem },
        { role: 'user', content: objective },
      ],
      schema: schemas.goals,
    });

    return result.structured.goals;
  });

const reviewWithSkills = (provider, objective, goals, skills) =>
  action('Reviewing goals with skills', async () => {
    const result = await provider.complete({
      model,
      temperature,
      messages: [
        { role: 'system', content: reviewSkillsSystem(objective, skills) },
        { role: 'user', content: goalsUser(goals) },
      ],
      schema: schemas.goals,
    });

    return result.structured.goals;
  });

const extractHints = (provider, objective, goals, skills) =>
  action('Extracting skill hints', async () => {
    const result = await provider.complete({
      model,
      temperature,
      messages: [
        { role: 'system', content: extractHintsSystem(objective, skills) },
        { role: 'user', content: goalsUser(goals) },
      ],
      schema: schemas.hints,
    });

    return result.structured.hints;
  });

const reviewWithHints = (provider, objective, goals, hints) =>
  action('Reviewing goals with hints', async () => {
    const result = await provider.complete({
      model,
      temperature,
      messages: [
        { role: 'system', content: reviewHintsSystem(objective, hints) },
        { role: 'user', content: goalsUser(goals) },
      ],
      schema: schemas.goals,
    });

    return result.structured.goals;
  });

const judgeComparison = (provider, objective, evaluation) =>
  action('Judging blind options', async () => {
    const { goals, optionA, optionB, skills } = evaluation;
    const result = await provider.complete({
      model: judgeModel,
      temperature,
      messages: [
        { role: 'system', content: judgeSystem(objective, skills) },
        { role: 'user', content: judgeUser({ goals, optionA, optionB }) },
      ],
      schema: schemas.judge,
    });

    return result.structured;
  });

const command = 'npm run llm:full-skill-vs-hints';
const help = `Usage:
  ${command}
  ${command} -- --resume .llm-lab/full-skill-vs-hints/runs/<run-id>.json
  ${command} -- --judge
  ${command} -- --judge --resume .llm-lab/full-skill-vs-hints/runs/<run-id>.json

New runs use LLM_LAB_ROUNDS (default: 3). Judge mode generates and evaluates every round automatically.`;

const configuredRounds = () => {
  const value = Number(process.env.LLM_LAB_ROUNDS ?? '3');
  if (Number.isSafeInteger(value) && value >= 1) return value;
  throw new Error('LLM_LAB_ROUNDS must be a positive integer.');
};

const createProvider = () => {
  const apiKey = process.env.OPENROUTER_API_KEY?.trim();
  if (!apiKey) throw new Error('OPENROUTER_API_KEY is required.');

  return createUnifiedProvider({
    transport: createFetchTransport(),
    apiKey,
    logger,
  });
};

const displayPath = (path) => relative(process.cwd(), path) || path;
const resumeMessage = (path, mode) =>
  `Resume with: ${command} -- ${mode === 'judge' ? '--judge ' : ''}--resume ${displayPath(path)}`;

const initializeRun = async (resume, mode) => {
  if (resume !== undefined) {
    const path = resolve(resume);
    const run = await loadRun(path);
    assertCompatibleRun(run, {
      mode,
      model,
      judgeModel,
      temperature,
      cases: runCases,
    });
    const resumed =
      run.status === 'completed' ? run : setRunStatus(run, 'running');
    await saveRun(path, resumed);
    return { path, run: resumed };
  }

  const run = createRun({
    mode,
    model,
    judgeModel,
    temperature,
    rounds: configuredRounds(),
    cases: runCases,
  });
  const path = resolve(checkpointDirectory, `${run.id}.json`);
  await saveRun(path, run);
  return { path, run };
};

const failureCode = (error) =>
  error instanceof ProviderErrorObject
    ? error.data.code
    : error instanceof DOMException && error.name === 'AbortError'
      ? 'aborted'
      : 'unexpected_error';

const prepareRound = async (provider, current, round, skills) => {
  const goals = await generateGoals(provider, current.objective);
  const [reviewed, hints] = await Promise.all([
    reviewWithSkills(provider, current.objective, goals, skills),
    extractHints(provider, current.objective, goals, skills),
  ]);
  const reviewedWithHints = await reviewWithHints(
    provider,
    current.objective,
    goals,
    hints,
  );
  const fullSkill = Math.random() < 0.5 ? 'a' : 'b';

  return {
    caseName: current.name,
    round,
    goals,
    optionA: fullSkill === 'a' ? reviewed : reviewedWithHints,
    optionB: fullSkill === 'b' ? reviewed : reviewedWithHints,
    fullSkill,
    skills,
  };
};

const showPendingRound = (pending, current, rounds) => {
  note(
    [
      `# Objective\n${current.objective}`,
      ...(current.source === undefined ? [] : [`# Source\n${current.source}`]),
      `# P0\n${pending.goals.map((goal, index) => `${index + 1}. ${goal}`).join('\n')}`,
      `# Option A\n${pending.optionA.map((goal, index) => `${index + 1}. ${goal}`).join('\n')}`,
      `# Option B\n${pending.optionB.map((goal, index) => `${index + 1}. ${goal}`).join('\n')}`,
    ].join('\n\n'),
    `${current.name} · ${pending.round}/${rounds}`,
  );
};

const askChoice = () =>
  select({
    message:
      'Which option correctly integrates relevant evidence while preserving the user objective?',
    options: [
      { value: 'a', label: 'Option A' },
      { value: 'b', label: 'Option B' },
      { value: 'both', label: 'Both integrate correctly' },
      { value: 'neither', label: 'Neither integrates correctly' },
    ],
  });

const askRationale = () =>
  text({
    message: 'Rationale',
    placeholder: 'Optional',
  });

const askFailureAction = () =>
  select({
    message: 'The round failed. Your previous progress is saved.',
    options: [
      { value: 'retry', label: 'Retry this round' },
      { value: 'pause', label: 'Pause and resume later' },
    ],
  });

const printResults = (run) => {
  const count = (winner) =>
    run.results.filter((result) => result.winner === winner).length;

  note(
    [
      ...run.results.map(
        ({ name, round, choice, rationale, winner }) =>
          `${name} ${round}/${run.config.rounds}: ${choice === 'a' || choice === 'b' ? `option ${choice.toUpperCase()}` : choice} → ${winner}\n  ${rationale}`,
      ),
      '',
      ...(run.config.mode === 'judge'
        ? [`Model: ${run.config.judgeModel}`]
        : []),
      `Full skill: ${count('full skill')}`,
      `Hints: ${count('hints')}`,
      `Both: ${count('both')}`,
      `Neither: ${count('neither')}`,
    ].join('\n'),
    run.config.mode === 'judge' ? 'Judge result' : 'Test result',
  );
};

const main = async () => {
  const { values } = parseArgs({
    options: {
      help: { type: 'boolean', short: 'h' },
      judge: { type: 'boolean' },
      resume: { type: 'string' },
    },
    strict: true,
  });
  if (values.help === true) {
    console.log(help);
    return;
  }

  const mode = values.judge === true ? 'judge' : 'human';
  const initialized = await initializeRun(values.resume, mode);
  let run = initialized.run;
  const path = initialized.path;
  let provider;
  const skillCache = new Map();
  const caseByName = new Map(cases.map((current) => [current.name, current]));

  intro(
    mode === 'judge'
      ? `Mosaic judge · ${judgeModel}`
      : 'Mosaic: full skill × hints',
  );
  note(
    `${displayPath(path)}\n${run.results.length} evaluations saved.`,
    'Run checkpoint',
  );

  const pause = async (failed) => {
    run = setRunStatus(run, 'paused');
    await saveRun(path, run);
    cancel(`Run paused. ${resumeMessage(path, mode)}`);
    if (failed) process.exitCode = 1;
  };

  try {
    while (nextRound(run) !== null) {
      const task = nextRound(run);
      const current = caseByName.get(task.caseName);
      log.info(`${current.name}: round ${task.round}/${run.config.rounds}`);

      if (run.pending === null) {
        try {
          let skills = skillCache.get(current.name);
          if (skills === undefined) {
            skills = await action(`Loading skills for ${current.name}`, () =>
              loadSkills(current),
            );
            skillCache.set(current.name, skills);
          }
          provider ??= createProvider();
          run = beginRound(
            run,
            await prepareRound(provider, current, task.round, skills),
          );
          await saveRun(path, run);
        } catch (error) {
          const code = failureCode(error);
          run = recordFailure(run, task, code);
          await saveRun(path, run);
          log.error(`Round failed (${code}).`);
          if (mode === 'judge') {
            await pause(true);
            return;
          }
          const recovery = await askFailureAction();
          if (recovery === 'retry') continue;
          await pause(true);
          return;
        }
      }

      if (mode === 'judge') {
        try {
          provider ??= createProvider();
          const judgment = await judgeComparison(
            provider,
            current.objective,
            run.pending,
          );
          run = completeJudgment(run, judgment);
          await saveRun(path, run);
          continue;
        } catch (error) {
          const code = failureCode(error);
          run = recordFailure(run, task, code);
          await saveRun(path, run);
          log.error(`Judge failed (${code}).`);
          await pause(true);
          return;
        }
      }

      showPendingRound(run.pending, current, run.config.rounds);
      if (run.pending.choice === undefined) {
        const choice = await askChoice();
        if (isCancel(choice)) {
          await pause(false);
          return;
        }
        run = recordChoice(run, choice);
        await saveRun(path, run);
      }

      const rationale = await askRationale();
      if (isCancel(rationale)) {
        await pause(false);
        return;
      }
      run = completeRound(run, rationale);
      await saveRun(path, run);
    }

    run = setRunStatus(run, 'completed');
    await saveRun(path, run);
    printResults(run);
    outro(`Run completed. Results saved to ${displayPath(path)}.`);
  } catch {
    await pause(true);
  }
};

await main();
