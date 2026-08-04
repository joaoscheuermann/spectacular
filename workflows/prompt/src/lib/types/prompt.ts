import type { Artifact } from 'artifacts';
import type { LlmProvider, ReasoningEffort } from 'llms';
import type { Sandbox } from 'sandbox';

export type PromptOpenQuestionOptions = readonly [
  string,
  string,
  string,
  ...string[],
];

export type PromptOpenQuestion = {
  readonly question: string;
  readonly impact?: string;
  readonly recommendation: string;
  readonly options: PromptOpenQuestionOptions;
};

export type PromptExplorationFact = {
  readonly fact: string;
  readonly evidencePaths: readonly string[];
};

export type PromptExploration = {
  readonly summary: string;
  readonly facts: readonly PromptExplorationFact[];
};

export type PromptIntent = {
  readonly goal: string;
  readonly scope: string;
};

export type PromptData = {
  readonly initialUserPrompt: string;
  readonly exploration?: PromptExploration;
  readonly requestUnderstanding?: string;
  readonly intent?: PromptIntent;
  readonly openQuestions?: readonly PromptOpenQuestion[];
  readonly productRequirements?: readonly string[];
  readonly technicalRequirements?: readonly string[];
};

export type PromptArtifact = Artifact<PromptData>;

export type PromptWorkflowOptions = {
  readonly provider: LlmProvider;
  readonly model: string;
  readonly effort?: ReasoningEffort;
  readonly workspaceRoot: string;
  readonly sandbox: Sandbox;
  readonly signal?: AbortSignal;
  readonly maxQuestions?: number;
};
