/** One reranked skill and the selector's diagnostic assessment. */
export interface SkillCandidate {
  readonly skillName: string;
  readonly score: number;
  readonly rank: number;
  readonly rationale: string;
}

/** The ordered skill names selected for one workflow goal. */
export interface OrderedBundle {
  readonly goalId: string;
  readonly skills: readonly string[];
  readonly selectionRationale: string;
}

/** Complete validated routing decision for one workflow goal. */
export interface RoutingTrace {
  readonly candidates: readonly SkillCandidate[];
  readonly bundle: OrderedBundle;
}
