#!/usr/bin/env node

import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import { aggregateComparisons, comparePlans } from './comparison.mjs';
import { loadInputs } from './inputs.mjs';
import { createOutput, readInputIdentity } from './output.mjs';
import { loadRejudgeInputs } from './rejudge-inputs.mjs';
import { createRuntime } from './runtime.mjs';
import { allFulfilled } from './utils.mjs';

const directory = dirname(fileURLToPath(import.meta.url));
const outcomes = ['withoutP0', 'withP0', 'both', 'neither', 'inconsistent'];
// All artifact and treatment validation intentionally precedes run creation.
const inputs = await loadInputs();
const inputIdentity = await readInputIdentity(directory);

const rejudge = await loadRejudgeInputs({
  directory,
  inputs,
  identity: inputIdentity,
});

const output = await createOutput({
  directory,
  config: rejudge.config,
  mode: 'rejudge',
  lineage: rejudge.lineage,
  fixtureSource: inputs.fixture.source,
  fixtureCases: inputs.fixture.cases.length,
  validatedInputIdentity: inputIdentity,
});
const { logger, action, createProvider, recordAttempt, recordUsage } =
  createRuntime(output);

const runCase = async (provider, current) => {
  const comparison = await comparePlans({
    provider,
    action,
    recordUsage,
    recordAttempt,
    config: rejudge.config,
    objective: current.objective,
    skills: current.goldSkills,
    plans: current.plans,
    options: current.options,
    operation: 'rejudge',
  });

  return {
    name: current.name,
    objective: current.objective,
    p0: current.p0,
    goldSkills: current.goldSkills.map(({ name }) => name),
    plans: current.plans,
    sourcePlanSha256: current.sourcePlanSha256,
    sourceRawOutcome: current.sourceRawOutcome,
    sourceOutcome: current.sourceOutcome,
    judgments: comparison.judgments,
    rawOutcome: comparison.rawOutcome,
    outcome: comparison.outcome,
    plansIdentical: comparison.plansIdentical,
  };
};

const agreementMetrics = (results) => {
  const matrix = Object.fromEntries(
    outcomes.map((source) => [
      source,
      Object.fromEntries(outcomes.map((target) => [target, 0])),
    ]),
  );

  for (const current of results) {
    matrix[current.sourceOutcome][current.outcome] += 1;
  }

  const exact = results.filter(
    ({ sourceOutcome, outcome }) => sourceOutcome === outcome,
  ).length;

  const stable = results.filter(
    ({ sourceOutcome, outcome }) =>
      sourceOutcome !== 'inconsistent' && outcome !== 'inconsistent',
  );

  const stableExact = stable.filter(
    ({ sourceOutcome, outcome }) => sourceOutcome === outcome,
  ).length;

  const decisive = results.filter(
    ({ sourceOutcome, outcome }) =>
      ['withoutP0', 'withP0'].includes(sourceOutcome) &&
      ['withoutP0', 'withP0'].includes(outcome),
  );

  const decisiveExact = decisive.filter(
    ({ sourceOutcome, outcome }) => sourceOutcome === outcome,
  ).length;

  return {
    outcomeMatrix: matrix,
    exactOutcomeAgreements: exact,
    exactOutcomeAgreementRate: exact / results.length,
    stablePairs: stable.length,
    stableOutcomeAgreements: stableExact,
    stableOutcomeAgreementRate:
      stable.length === 0 ? null : stableExact / stable.length,
    decisivePairs: decisive.length,
    decisiveDirectionAgreements: decisiveExact,
    decisiveDirectionAgreementRate:
      decisive.length === 0 ? null : decisiveExact / decisive.length,
  };
};

const corpusMetrics = (results) => ({
  localCases: inputs.cases.length,
  catalogSkills: inputs.catalog.length,
  frozenP0s: inputs.fixture.cases.length,
  frozenP0Goals: inputs.cases.reduce(
    (total, current) => total + current.p0.length,
    0,
  ),
  frozenPlanPairs: rejudge.cases.length,
  goldBundles: results.length,
  goldSkillOccurrences: results.reduce(
    (total, current) => total + current.goldSkills.length,
    0,
  ),
  uniqueGoldSkills: new Set(results.flatMap(({ goldSkills }) => goldSkills))
    .size,
  expectedSuccessfulCalls: { rejudge: 60, total: 60 },
});

const main = async () => {
  const provider = createProvider();

  logger.info(
    {
      runId: output.id,
      sourceRunId: rejudge.lineage.sourceRunId,
      sourceJudgeModel: rejudge.lineage.sourceJudgeModel,
      targetJudgeModel: rejudge.config.judgeModel,
      cases: rejudge.cases.length,
    },
    'Rejudge run started',
  );

  const results = await allFulfilled(
    rejudge.cases.map((current) => runCase(provider, current)),
    'One or more rejudge cases failed.',
  );

  const metrics = {
    corpus: corpusMetrics(results),
    sourceComparison: rejudge.sourceComparison,
    comparison: aggregateComparisons(results, rejudge.config.judgeModel),
    crossJudgeAgreement: agreementMetrics(results),
  };

  await output.complete(results, metrics);

  logger.info(
    { runId: output.id, cases: results.length, metrics },
    'Rejudge run completed',
  );
};

try {
  await main();
} catch (error) {
  logger.fatal({ error, runId: output.id }, 'Rejudge run failed');

  await output.fail();

  process.exitCode = 1;
}
