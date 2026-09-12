#!/usr/bin/env node

import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import {
  aggregateExposureComparisons,
  comparePlans,
  comparisonOutcome,
  comparisonProtocolSha256,
  orientations,
} from './comparison.mjs';
import {
  controlFixtureName,
  defaultFixtureName,
  loadInputs,
} from './inputs.mjs';
import { createOutput } from './output.mjs';
import { directPlanSystem, withP0User } from './prompt.mjs';
import { createRuntime } from './runtime.mjs';
import { goalsSchema } from './schemas.mjs';
import { messages } from './utils.mjs';

const directory = dirname(fileURLToPath(import.meta.url));

const { values } = parseArgs({
  options: { fixture: { type: 'string', default: defaultFixtureName } },
  strict: true,
  allowPositionals: false,
});
const fixtureName = values.fixture.trim();

if (fixtureName.length === 0) {throw new Error('--fixture must not be empty.');}

const config = {
  treatment: 'exposure',
  design: 'frozen_control',
  adjudication: {
    trigger: 'inconsistent',
    extraOrientations: 2,
    consensus: 'strict_majority',
  },
  planningModel: 'qwen/qwen3.8-27b',
  planningEffort: 'high',
  judgeModel: 'openai/gpt-5.6-sol',
  judgeEffort: 'medium',
  comparisonProtocolSha256: comparisonProtocolSha256(),
  retry: { attempts: 5, delayMs: 15_000, backoffMultiplier: 2 },
};
// Input and fixture validation intentionally precedes provider construction.
const inputs = await loadInputs({ fixtureName, withControl: true });

const output = await createOutput({
  directory,
  config,
  mode: 'generation',
  fixtureSource: inputs.fixture.source,
  fixtureCases: inputs.fixture.cases.length,
  fixtureName: inputs.fixtureName,
  controlSource: inputs.controlFixture.source,
  controlCases: inputs.controlFixture.cases.length,
  controlFixtureName: inputs.controlFixtureName,
});
const { logger, action, createProvider, recordAttempt, recordUsage } =
  createRuntime(output);

const completePlan = async (
  provider,
  current,
  operation,
  systemPrompt,
  userMessage,
) =>
  action(
    `Generating ${operation} for ${current.name}`,
    async () => {
      recordAttempt(operation, config.planningModel);

      const result = await provider.complete({
        model: config.planningModel,
        effort: config.planningEffort,
        messages: messages(systemPrompt, userMessage),
        schema: goalsSchema,
        flags: { sensitiveOutput: true },
      });

      recordUsage(operation, config.planningModel, result.usage);

      return result.structured.goals;
    },
    config.retry,
  );

const runCase = async (provider, current) => {
  const withP0 = await completePlan(
    provider,
    current,
    'plan_with_p0',
    directPlanSystem,
    withP0User(current.objective, current.goldSkills, current.p0),
  );
  const plans = { withoutP0: current.controlPlan, withP0 };

  const initial = await comparePlans({
    provider,
    action,
    recordUsage,
    recordAttempt,
    config,
    objective: current.objective,
    skills: current.goldSkills,
    plans,
    options: orientations(plans),
  });

  const adjudication =
    initial.outcome === 'inconsistent'
      ? await comparePlans({
          provider,
          action,
          recordUsage,
          recordAttempt,
          config,
          objective: current.objective,
          skills: current.goldSkills,
          plans,
          options: orientations(plans),
          operation: 'judge_adjudication',
          orientationOffset: 2,
        })
      : undefined;
  const judgments = [...initial.judgments, ...(adjudication?.judgments ?? [])];
  const comparison = comparisonOutcome(plans, judgments);

  return {
    name: current.name,
    objective: current.objective,
    p0: current.p0,
    goldSkills: current.goldSkills.map(({ name }) => name),
    plans,
    judgments,
    rawOutcome: comparison.rawOutcome,
    outcome: comparison.outcome,
    plansIdentical: comparison.plansIdentical,
  };
};

const corpusMetrics = (results) => {
  const uniqueGoldSkills = new Set(
    results.flatMap(({ goldSkills }) => goldSkills),
  );

  const adjudicatedCases = results.filter(
    ({ judgments }) => judgments.length === 4,
  ).length;

  return {
    localCases: inputs.cases.length,
    catalogSkills: inputs.catalog.length,
    frozenP0s: inputs.fixture.cases.length,
    frozenP0Goals: inputs.cases.reduce(
      (total, current) => total + current.p0.length,
      0,
    ),
    frozenControlPlans: inputs.controlFixture.cases.length,
    frozenControlGoals: inputs.cases.reduce(
      (total, current) => total + current.controlPlan.length,
      0,
    ),
    goldBundles: results.length,
    goldSkillOccurrences: results.reduce(
      (total, current) => total + current.goldSkills.length,
      0,
    ),
    uniqueGoldSkills: uniqueGoldSkills.size,
    successfulCallContract: {
      fixed: {
        plan_with_p0: 30,
        judge: 60,
        total: 90,
      },
      conditional: {
        judge_adjudicationPerInitialInconsistentCase: 2,
        maximumAdditionalCalls: 60,
        maximumTotal: 150,
      },
    },
    expectedSuccessfulCallsForThisRun: {
      plan_with_p0: 30,
      judge: 60,
      judge_adjudication: adjudicatedCases * 2,
      total: 90 + adjudicatedCases * 2,
    },
  };
};

const main = async () => {
  const provider = createProvider();

  logger.info(
    {
      runId: output.id,
      cases: inputs.cases.length,
      fixture: inputs.fixtureName,
      sourceRunId: inputs.fixture.source.runId,
      controlFixture: inputs.controlFixtureName,
      controlSourceRunId: inputs.controlFixture.source.runId,
    },
    'Run started',
  );

  const results = await Promise.all(
    inputs.cases.map((current) => runCase(provider, current)),
  );

  const metrics = {
    corpus: corpusMetrics(results),
    comparison: aggregateExposureComparisons(results, config.judgeModel),
  };

  await output.complete(results, metrics);

  logger.info(
    { runId: output.id, cases: results.length, metrics },
    'Run completed',
  );
};

try {
  await main();
} catch (error) {
  logger.fatal({ error, runId: output.id }, 'Run failed');

  await output.fail();

  process.exitCode = 1;
}
