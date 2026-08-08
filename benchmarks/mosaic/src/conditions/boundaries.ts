import type { Case, Condition, ExecutionRecord } from '../schemas/index.js';

export type EvaluationBoundary =
  | 'initialPlan'
  | 'feedbackPlan'
  | 'skillView'
  | 'retrieval'
  | 'routing'
  | 'menu'
  | 'execution'
  | 'localizedRevision';

export interface BoundaryPolicy {
  readonly initialPlan: 'default' | 'oracle';
  readonly feedbackPlan: 'default' | 'unchanged';
  readonly skillView: 'none' | 'metadata' | 'body';
  readonly retrieval: 'none' | 'default' | 'oracle';
  readonly routing: 'none' | 'single' | 'selective' | 'top-k' | 'oracle';
  readonly bundleOrder: 'not-applicable' | 'ranked' | 'shuffled';
  readonly menu: 'global' | 'declared' | 'base-plus-bundle' | 'oracle';
  readonly execution: 'default' | 'oracle-state';
  readonly localizedRevision: 'disabled' | 'default' | 'oracle';
}

/** Maps frozen condition factors and diagnostic IDs to evaluation-hook boundaries. */
export const boundaryPolicy = (condition: Condition): BoundaryPolicy => ({
  initialPlan: condition.id === 'O_PLAN' ? 'oracle' : 'default',
  feedbackPlan: condition.factors.catalogFeedback ? 'default' : 'unchanged',
  skillView: condition.factors.skillView,
  retrieval:
    condition.factors.retrieval === 'oracle'
      ? 'oracle'
      : condition.factors.retrieval === 'none'
        ? 'none'
        : 'default',
  routing:
    condition.id === 'O_BUNDLE' || condition.factors.bundle === 'oracle'
      ? 'oracle'
      : condition.factors.bundle,
  bundleOrder: condition.factors.bundleOrder,
  menu: condition.factors.menu,
  execution: condition.id === 'O_STATE' ? 'oracle-state' : 'default',
  localizedRevision:
    condition.id === 'O_REVISION'
      ? 'oracle'
      : condition.factors.localizedRevision
        ? 'default'
        : 'disabled',
});

/** Enforces that diagnostic oracles are scheduled only for failed parent runs. */
export const oracleEligible = (
  condition: Condition,
  benchmarkCase: Case,
  parent: ExecutionRecord | undefined,
): boolean => {
  void benchmarkCase;
  if (condition.eligibility !== 'failures-only') return true;
  return parent?.status === 'failed';
};
