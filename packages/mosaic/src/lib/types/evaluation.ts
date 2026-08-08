import type { Skill } from 'bundle';
import type { Tool } from 'tool';

import type { NodeDecision } from '../schemas/outcome.js';
import type { PlannedGraph } from '../schemas/graph.js';
import type { Graph, Node } from './graph.js';
import type { Observation } from './revision.js';
import type { RoutingTrace } from './routing.js';

export type EvaluationHook<Input, Output> = (
  input: Readonly<Input>,
  next: (input: Readonly<Input>) => Promise<Output>,
) => Promise<Output>;

export interface InitialPlanInput {
  readonly request: string;
}

export interface FeedbackPlanInput {
  readonly request: string;
  readonly graph: Graph;
}

export type FeedbackPlanResult = PlannedGraph | 'unchanged';

export interface SkillViewInput {
  readonly skill: Skill;
  readonly purpose: 'hint' | 'routing';
  readonly nodeId: string;
}

export interface SkillMatch {
  readonly skill: Skill;
  readonly score: number;
}

export interface RetrievalInput {
  readonly request: string;
  readonly graph: Graph;
  readonly node: Node;
  readonly query: string;
  readonly limit: number;
  readonly catalog: readonly Skill[];
}

export interface RoutingInput {
  readonly request: string;
  readonly graph: Graph;
  readonly node: Node;
}

export interface MenuInput {
  readonly node: Node;
  readonly trace: RoutingTrace;
  readonly skills: readonly Skill[];
  readonly required: readonly Tool[];
  readonly catalog: readonly Tool[];
}

export interface ExecutionInput {
  readonly request: string;
  readonly graph: Graph;
  readonly node: Node;
  readonly skills: readonly Skill[];
  readonly tools: readonly Tool[];
}

export interface ExecutionResult {
  readonly decision: NodeDecision;
  readonly observations: readonly Observation[];
}

export interface LocalizedRevisionInput {
  readonly request: string;
  readonly graph: Graph;
  readonly target: Node;
  readonly retiredIds: readonly string[];
}

export interface MosaicEvaluationHooks {
  readonly initialPlan?: EvaluationHook<InitialPlanInput, PlannedGraph>;
  readonly feedbackPlan?: EvaluationHook<FeedbackPlanInput, FeedbackPlanResult>;
  readonly skillView?: EvaluationHook<SkillViewInput, string>;
  readonly retrieval?: EvaluationHook<RetrievalInput, readonly SkillMatch[]>;
  readonly routing?: EvaluationHook<RoutingInput, RoutingTrace>;
  readonly menu?: EvaluationHook<MenuInput, readonly Tool[]>;
  readonly execution?: EvaluationHook<ExecutionInput, ExecutionResult>;
  readonly localizedRevision?: EvaluationHook<
    LocalizedRevisionInput,
    PlannedGraph
  >;
}

export interface MosaicEvaluationOptions {
  readonly hooks?: MosaicEvaluationHooks;
}
