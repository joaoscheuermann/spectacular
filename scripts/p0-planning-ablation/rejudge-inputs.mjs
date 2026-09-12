import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { parseArgs } from 'node:util';

import {
  aggregateComparisons,
  comparisonOutcome,
  comparisonProtocolSha256,
  orientation,
  outcomeForChoice,
} from './comparison.mjs';
import {
  generationManifestSchema,
  generationResultsSchema,
  rejudgeCampaignSchema,
} from './schemas.mjs';

const campaignPath = (directory) => join(directory, 'fixtures', 'rejudge.json');

const sha256 = (data) => createHash('sha256').update(data).digest('hex');

const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);

const readPinnedJson = async (path, expectedSha256, schema, label) => {
  const data = await readFile(path);
  const actualSha256 = sha256(data);

  if (actualSha256 !== expectedSha256) {
    throw new Error(
      `${label} SHA-256 mismatch: expected ${expectedSha256}, found ${actualSha256}.`,
    );
  }

  return schema.parse(JSON.parse(data.toString('utf8')));
};

const parseCli = (argv) => {
  const { values } = parseArgs({
    args: argv,
    options: {
      'source-run': { type: 'string' },
      'judge-model': { type: 'string' },
    },
    strict: true,
    allowPositionals: false,
  });
  const sourceRunId = values['source-run']?.trim();
  const judgeModel = values['judge-model']?.trim();

  if (!sourceRunId || !judgeModel) {
    throw new Error(
      'Both --source-run and --judge-model are required for rejudge.',
    );
  }

  return { sourceRunId, judgeModel };
};

const loadCampaign = async (directory) => {
  const data = await readFile(campaignPath(directory));

  const campaign = rejudgeCampaignSchema.parse(
    JSON.parse(data.toString('utf8')),
  );
  const runIds = campaign.sources.map(({ runId }) => runId);

  if (new Set(runIds).size !== runIds.length) {
    throw new Error('Rejudge campaign contains duplicate source runs.');
  }

  const protocolSha256 = comparisonProtocolSha256();

  if (campaign.comparisonProtocolSha256 !== protocolSha256) {
    throw new Error(
      `Comparison protocol SHA-256 mismatch: expected ${campaign.comparisonProtocolSha256}, found ${protocolSha256}.`,
    );
  }

  return { campaign, campaignSha256: sha256(data), protocolSha256 };
};

const findCampaignCell = (campaign, sourceRunId, judgeModel) => {
  const source = campaign.sources.find(({ runId }) => runId === sourceRunId);

  if (source === undefined) {
    throw new Error(
      `Source run is not pinned by the campaign: ${sourceRunId}.`,
    );
  }

  if (source.targetJudgeModel !== judgeModel) {
    throw new Error(
      `Campaign requires judge ${source.targetJudgeModel} for source ${sourceRunId}; received ${judgeModel}.`,
    );
  }

  if (source.sourceJudgeModel === source.targetJudgeModel) {
    throw new Error(`Rejudge target must differ from the source judge.`);
  }

  if (
    source.attestedComparisonProtocolSha256 !==
    campaign.comparisonProtocolSha256
  ) {
    throw new Error(`Source protocol attestation differs from the campaign.`);
  }

  return source;
};

const assertSourceIdentity = (manifest, results, source, inputs, identity) => {
  if (manifest.runId !== source.runId || results.runId !== source.runId) {
    throw new Error(`Source run ID mismatch for ${source.runId}.`);
  }

  if (manifest.config.judgeModel !== source.sourceJudgeModel) {
    throw new Error(`Source judge mismatch for ${source.runId}.`);
  }

  if (manifest.config.judgeEffort !== source.targetJudgeEffort) {
    throw new Error(`Rejudge effort differs from source for ${source.runId}.`);
  }

  if (
    manifest.identity.experimentSources.sha256 !==
    source.sourceExperimentSourcesSha256
  ) {
    throw new Error(`Source-code identity mismatch for ${source.runId}.`);
  }

  if (
    manifest.identity.fixture.runId !== inputs.fixture.source.runId ||
    manifest.identity.fixture.sourceResultsSha256 !==
      inputs.fixture.source.resultsSha256 ||
    manifest.identity.fixture.sha256 !== identity.fixture.sha256 ||
    !same(manifest.identity.cases, identity.cases) ||
    !same(manifest.identity.catalog, identity.catalog)
  ) {
    throw new Error(`Source inputs do not match current frozen inputs.`);
  }

  if (manifest.cases !== results.cases) {
    throw new Error(`Source manifest/results case count mismatch.`);
  }
};

const assertSourceJudgments = (current) => {
  const [first, second] = current.judgments;

  if (
    first.orientation !== 1 ||
    second.orientation !== 2 ||
    first.withoutP0Option === second.withoutP0Option
  ) {
    throw new Error(`Invalid source orientations for ${current.name}.`);
  }

  for (const judgment of current.judgments) {
    const expectedWinner = outcomeForChoice(
      judgment.withoutP0Option,
      judgment.choice,
    );

    if (judgment.winner !== expectedWinner) {
      throw new Error(`Invalid source winner mapping for ${current.name}.`);
    }
  }

  const recomputed = comparisonOutcome(current.plans, current.judgments);

  if (
    recomputed.rawOutcome !== current.rawOutcome ||
    recomputed.outcome !== current.outcome ||
    recomputed.plansIdentical !== current.plansIdentical
  ) {
    throw new Error(`Invalid persisted source outcome for ${current.name}.`);
  }
};

const resolveCases = (inputs, sourceResults) => {
  const sourceByName = new Map();

  for (const current of sourceResults.results) {
    if (sourceByName.has(current.name)) {
      throw new Error(`Duplicate source case: ${current.name}.`);
    }

    sourceByName.set(current.name, current);
  }

  const localNames = new Set(inputs.cases.map(({ name }) => name));

  const unknown = [...sourceByName.keys()].filter(
    (name) => !localNames.has(name),
  );

  if (unknown.length > 0) {
    throw new Error(`Unknown source cases: ${unknown.join(', ')}.`);
  }

  return inputs.cases.map((local) => {
    const source = sourceByName.get(local.name);

    if (source === undefined) {
      throw new Error(`Missing source case: ${local.name}.`);
    }

    const goldSkills = local.goldSkills.map(({ name }) => name);

    if (
      source.objective !== local.objective ||
      !same(source.p0, local.p0) ||
      !same(source.goldSkills, goldSkills)
    ) {
      throw new Error(`Source treatment mismatch for ${local.name}.`);
    }

    assertSourceJudgments(source);

    return Object.freeze({
      name: local.name,
      objective: local.objective,
      p0: local.p0,
      goldSkills: local.goldSkills,
      plans: source.plans,
      options: Object.freeze(
        source.judgments.map(({ withoutP0Option }) =>
          Object.freeze(orientation(source.plans, withoutP0Option)),
        ),
      ),
      sourcePlanSha256: sha256(JSON.stringify(source.plans)),
      sourceOutcome: source.outcome,
      sourceRawOutcome: source.rawOutcome,
      plansIdentical: source.plansIdentical,
    });
  });
};

const assertSourceSummary = (sourceResults, judgeModel) => {
  const actual = aggregateComparisons(sourceResults.results, judgeModel);

  if (!same(actual, sourceResults.metrics?.comparison)) {
    throw new Error('Source comparison metrics do not match case outcomes.');
  }
};

/** Loads and validates one fixed cross-judge cell without provider access. */
export const loadRejudgeInputs = async ({
  directory,
  inputs,
  identity,
  argv = process.argv.slice(2),
}) => {
  const { sourceRunId, judgeModel } = parseCli(argv);
  const { campaign, campaignSha256, protocolSha256 } =
    await loadCampaign(directory);
  const source = findCampaignCell(campaign, sourceRunId, judgeModel);
  const sourceDirectory = join(directory, 'output', source.runId);

  const [manifest, sourceResults] = await Promise.all([
    readPinnedJson(
      join(sourceDirectory, 'manifest.json'),
      source.manifestSha256,
      generationManifestSchema,
      'Source manifest',
    ),
    readPinnedJson(
      join(sourceDirectory, 'results.json'),
      source.resultsSha256,
      generationResultsSchema,
      'Source results',
    ),
  ]);

  assertSourceIdentity(manifest, sourceResults, source, inputs, identity);

  const cases = resolveCases(inputs, sourceResults);

  assertSourceSummary(sourceResults, source.sourceJudgeModel);

  return Object.freeze({
    config: Object.freeze({
      judgeModel: source.targetJudgeModel,
      judgeEffort: source.targetJudgeEffort,
      comparisonProtocolSha256: protocolSha256,
      retry: manifest.config.retry,
    }),
    lineage: Object.freeze({
      type: 'cross_judge_rejudge',
      sourceRunId: source.runId,
      sourceManifestSha256: source.manifestSha256,
      sourceResultsSha256: source.resultsSha256,
      sourceJudgeModel: source.sourceJudgeModel,
      sourceJudgeEffort: manifest.config.judgeEffort,
      sourcePlanningModel: manifest.config.planningModel,
      sourcePlanningEffort: manifest.config.planningEffort,
      sourceExperimentSourcesSha256: manifest.identity.experimentSources.sha256,
      sourcePackageMetadataSha256: manifest.identity.packageMetadata.sha256,
      sourceProtocolAttestation: 'reviewed_source_reconstruction',
      targetJudgeModel: source.targetJudgeModel,
      targetJudgeEffort: source.targetJudgeEffort,
      comparisonProtocolSha256: protocolSha256,
      campaignSha256,
    }),
    cases: Object.freeze(cases),
    sourceComparison: sourceResults.metrics.comparison,
  });
};
