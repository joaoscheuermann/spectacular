import { note } from '@clack/prompts';

import { armLabels, pairLabel } from './comparison.mjs';

const numbered = (goals) =>
  goals.map((goal, index) => `${index + 1}. ${goal}`).join('\n');

export const showComparison = (
  current,
  round,
  rounds,
  pair,
  orientation,
  options,
) => {
  note(
    `# Objective
${current.objective}

# Option A
${numbered(options.optionA)}

# Option B
${numbered(options.optionB)}`,
    `${current.name} · ${round}/${rounds} · ${pairLabel(pair)} · orientation ${orientation}/2`,
  );
};

const vectorTrace = (shortlist) =>
  shortlist.map(({ data, score }, index) => ({
    rank: index + 1,
    skill: data.name,
    vectorScore: score,
  }));

const rankingTrace = (ranked) =>
  ranked.map(({ skill, vectorScore, rerankerScore, rank }) => ({
    rank,
    skill: skill.name,
    vectorScore,
    rerankerScore,
  }));

const searchTrace = ({ shortlist, reranked, selected }) => ({
  vectorShortlist: vectorTrace(shortlist),
  reranked: rankingTrace(reranked),
  selected: rankingTrace(selected),
});

export const retrievalTrace = (plans) => ({
  request: searchTrace(plans.requestRanking),
  goals: plans.byGoal.map(({ goal, shortlist, reranked, selected }) => ({
    goal,
    ...searchTrace({ shortlist, reranked, selected }),
  })),
});

const formatVector = (shortlist) =>
  shortlist
    .map(
      ({ skill, vectorScore, rank }) =>
        `${rank}. ${skill} (${vectorScore.toFixed(4)})`,
    )
    .join(', ');

const formatRanking = (ranked) =>
  ranked.length === 0
    ? 'none'
    : ranked
        .map(
          ({ skill, vectorScore, rerankerScore, rank }) =>
            `${rank}. ${skill} (vector=${vectorScore.toFixed(4)}, reranker=${rerankerScore.toFixed(6)})`,
        )
        .join(', ');

const outcomeLabel = (winner) => armLabels[winner] ?? winner;

const formatJudgment = (pair, judgment) => {
  const { orientation, leftOption, choice, rationale, winner } = judgment;

  const selection =
    choice === 'a' || choice === 'b'
      ? `option ${choice.toUpperCase()}`
      : choice;

  return `  Orientation ${orientation} (${armLabels[pair.left]}=${leftOption.toUpperCase()}): ${selection} → ${outcomeLabel(winner)}
    ${rationale}`;
};

const failureDetails = (failure) =>
  [
    failure.code,
    failure.httpStatus === undefined ? null : `HTTP ${failure.httpStatus}`,
    failure.provider === undefined ? null : `provider=${failure.provider}`,
    failure.operation === undefined ? null : `operation=${failure.operation}`,
    failure.model === undefined ? null : `model=${failure.model}`,
    failure.retryable === undefined ? null : `retryable=${failure.retryable}`,
  ]
    .filter((detail) => detail !== null)
    .join(', ');

const formatFailure = (pair, failure) => {
  const unit = failure.attempts === 1 ? 'attempt' : 'attempts';

  return `  Orientation ${failure.orientation} (${armLabels[pair.left]}=${failure.leftOption.toUpperCase()}): failed after ${failure.attempts} ${unit} [${failureDetails(failure)}]`;
};

const formatComparison = (comparison) => {
  const { judgeModel, status, judgments, failures = [], winner } = comparison;
  const result = status === 'completed' ? outcomeLabel(winner) : status;

  const details = [
    ...judgments.map((judgment) => formatJudgment(comparison, judgment)),
    ...failures.map((failure) => formatFailure(comparison, failure)),
  ].join('\n');

  return `${judgeModel ?? 'Human'} · ${pairLabel(comparison)}: ${result}
${details}`;
};

const formatSearch = (
  label,
  search,
) => `${label} vector shortlist: ${formatVector(search.vectorShortlist)}
${label} reranked: ${formatRanking(search.reranked)}
${label} selected: ${formatRanking(search.selected)}`;

const formatResult = (
  result,
  rounds,
) => `${result.name} ${result.round}/${rounds}
${result.comparisons.map(formatComparison).join('\n')}
${formatSearch('Request', result.retrieval.request)}
${result.retrieval.goals
  .map((search, index) => formatSearch(`Goal ${index + 1}`, search))
  .join('\n')}`;

const comparisonSummary = (run, pair, judgeModel) => {
  const comparisons = run.results.flatMap((result) =>
    result.comparisons.filter(
      (comparison) =>
        comparison.id === pair.id && comparison.judgeModel === judgeModel,
    ),
  );
  const count = (winner) =>
    comparisons.filter((comparison) => comparison.winner === winner).length;

  return `${pairLabel(pair)}
  ${armLabels[pair.left]}: ${count(pair.left)}
  ${armLabels[pair.right]}: ${count(pair.right)}
  Both: ${count('both')}
  Neither: ${count('neither')}
  Inconsistent: ${count('inconsistent')}
  Partial: ${comparisons.filter(({ status }) => status === 'partial').length}
  Failed: ${comparisons.filter(({ status }) => status === 'failed').length}`;
};

export const printResults = (run) => {
  const summaries =
    run.config.mode === 'judge'
      ? run.config.judgeModels.flatMap((judgeModel) => [
          `Judge: ${judgeModel}`,
          ...run.config.comparisonPairs.map((pair) =>
            comparisonSummary(run, pair, judgeModel),
          ),
        ])
      : run.config.comparisonPairs.map((pair) =>
          comparisonSummary(run, pair, null),
        );

  note(
    [
      ...run.results.map((result) => formatResult(result, run.config.rounds)),
      '',
      ...summaries,
    ].join('\n'),
    run.config.mode === 'judge' ? 'Judge results' : 'Test result',
  );
};
