#!/usr/bin/env node

import { dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  aggregateComparisons,
  comparePlans,
  comparisonProtocolSha256,
  orientations,
} from './comparison.mjs';
import { loadInputs } from './inputs.mjs';
import { createOutput } from './output.mjs';
import {
  directPlanSystem,
  p1WithoutP0User,
  p1WithP0User,
  revisionPlanSystem,
} from './prompt.mjs';
import { createRuntime } from './runtime.mjs';
import { goalsSchema } from './schemas.mjs';
import { messages } from './utils.mjs';

const directory = dirname(fileURLToPath(import.meta.url));
const config = {
  planningModel: 'qwen/qwen3.8-27b',
  planningEffort: 'high',
  judgeModel: 'openai/gpt-6-astra',
  judgeEffort: 'medium',
  comparisonProtocolSha256: comparisonProtocolSha256(),
  retry: { attempts: 5, delayMs: 15_000, backoffMultiplier: 2 },
};

// Input and fixture validation intentionally precedes provider construction.
const inputs = await loadInputs();
const output = await createOutput({
  directory,
  config,
  mode: 'generation',
  fixtureSource: inputs.fixture.source,
  fixtureCases: inputs.fixture.cases.length,
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
  const [withoutP0, withP0] = await Promise.all([
    completePlan(
      provider,
      current,
      'p1_without_p0',
      directPlanSystem,
      p1WithoutP0User(current.objective, current.goldSkills),
    ),
    completePlan(
      provider,
      current,
      'p1_with_p0',
      revisionPlanSystem,
      p1WithP0User(current.objective, current.goldSkills, current.p0),
    ),
  ]);
  const plans = { withoutP0, withP0 };
  const comparison = await comparePlans({
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

  return {
    name: current.name,
    objective: current.objective,
    p0: current.p0,
    goldSkills: current.goldSkills.map(({ name }) => name),
    plans,
    judgments: comparison.judgments,
    rawOutcome: comparison.rawOutcome,
    outcome: comparison.outcome,
    plansIdentical: comparison.plansIdentical,
  };
};

const corpusMetrics = (results) => {
  const uniqueGoldSkills = new Set(
    results.flatMap(({ goldSkills }) => goldSkills),
  );
  return {
    localCases: inputs.cases.length,
    catalogSkills: inputs.catalog.length,
    frozenP0s: inputs.fixture.cases.length,
    frozenP0Goals: inputs.cases.reduce(
      (total, current) => total + current.p0.length,
      0,
    ),
    goldBundles: results.length,
    goldSkillOccurrences: results.reduce(
      (total, current) => total + current.goldSkills.length,
      0,
    ),
    uniqueGoldSkills: uniqueGoldSkills.size,
    expectedSuccessfulCalls: {
      p1_without_p0: 30,
      p1_with_p0: 30,
      judge: 60,
      total: 120,
    },
  };
};

const main = async () => {
  const provider = createProvider();
  logger.info(
    {
      runId: output.id,
      cases: inputs.cases.length,
      sourceRunId: inputs.fixture.source.runId,
    },
    'Run started',
  );
  const results = await Promise.all(
    inputs.cases.map((current) => runCase(provider, current)),
  );
  const metrics = {
    corpus: corpusMetrics(results),
    comparison: aggregateComparisons(results, config.judgeModel),
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
