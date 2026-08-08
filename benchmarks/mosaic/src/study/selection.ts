export interface PilotOutcome {
  readonly conditionId: 'B0' | 'B1' | 'B2' | 'B3';
  readonly success: 0 | 1;
  readonly costUsd: number;
}

export interface BaselineSummary {
  readonly conditionId: PilotOutcome['conditionId'];
  readonly successRate: number;
  readonly meanCostUsd: number;
  readonly runs: number;
}

/** Selects by success, then all-run cost, then lexical condition ID. */
export const selectBaseline = (
  outcomes: readonly PilotOutcome[],
): BaselineSummary => {
  const ids = ['B0', 'B1', 'B2', 'B3'] as const;
  const summaries = ids.map((conditionId) => {
    const rows = outcomes.filter(
      (outcome) => outcome.conditionId === conditionId,
    );
    if (rows.length === 0)
      throw new TypeError(`missing pilot outcomes for ${conditionId}`);
    if (rows.some((row) => !Number.isFinite(row.costUsd) || row.costUsd < 0)) {
      throw new TypeError(`invalid pilot cost for ${conditionId}`);
    }
    return {
      conditionId,
      successRate:
        rows.reduce((sum, row) => sum + row.success, 0) / rows.length,
      meanCostUsd:
        rows.reduce((sum, row) => sum + row.costUsd, 0) / rows.length,
      runs: rows.length,
    };
  });
  if (new Set(summaries.map((summary) => summary.runs)).size !== 1) {
    throw new TypeError('baseline conditions require equal run counts');
  }
  return summaries.sort(
    (left, right) =>
      right.successRate - left.successRate ||
      left.meanCostUsd - right.meanCostUsd ||
      (left.conditionId < right.conditionId
        ? -1
        : left.conditionId > right.conditionId
          ? 1
          : 0),
  )[0] as BaselineSummary;
};
