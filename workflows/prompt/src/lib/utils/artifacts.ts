import type {
  PromptArtifact,
  PromptData,
  PromptExploration,
  PromptOpenQuestion,
} from '../types/prompt.js';

export const normalizeArtifact = (artifact: PromptArtifact): PromptArtifact => ({
  name: artifact.name || 'PROMPT',
  mime: artifact.mime || 'text/markdown',
  data: normalizeData(artifact.data),
  template: artifact.template,
});

export const withData = (
  artifact: PromptArtifact,
  data: Partial<PromptData>,
): PromptArtifact => ({
  ...artifact,
  data: {
    ...artifact.data,
    ...data,
  },
});

export const withoutRequirements = (artifact: PromptArtifact): PromptArtifact => {
  const { productRequirements, technicalRequirements, ...data } = artifact.data;

  return {
    ...artifact,
    data,
  };
};

const normalizeData = (data: PromptData): PromptData => ({
  initialUserPrompt: data.initialUserPrompt,
  ...(data.exploration === undefined
    ? {}
    : { exploration: cloneExploration(data.exploration) }),
  ...(data.requestUnderstanding === undefined
    ? {}
    : { requestUnderstanding: data.requestUnderstanding }),
  ...(data.intent === undefined ? {} : { intent: { ...data.intent } }),
  ...(data.openQuestions === undefined
    ? {}
    : { openQuestions: cloneQuestions(data.openQuestions) }),
  ...(data.productRequirements === undefined
    ? {}
    : { productRequirements: [...data.productRequirements] }),
  ...(data.technicalRequirements === undefined
    ? {}
    : { technicalRequirements: [...data.technicalRequirements] }),
});

const cloneExploration = (value: PromptExploration): PromptExploration => ({
  summary: value.summary,
  facts: value.facts.map((fact) => ({
    fact: fact.fact,
    evidencePaths: [...fact.evidencePaths],
  })),
});

const cloneQuestions = (
  questions: readonly PromptOpenQuestion[],
): readonly PromptOpenQuestion[] =>
  questions.map((question) => ({ ...question }));
