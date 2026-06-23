import type {
  PromptData,
  PromptExploration,
  PromptOpenQuestion,
} from '../types/prompt.js';

export const renderPromptData = (data: PromptData): string => `# Prompt

## Original prompt

${data.initialUserPrompt}

## Exploration

${data.exploration?.summary ?? 'None'}

${factList(data.exploration?.facts ?? [])}

## Request understanding

${data.requestUnderstanding ?? 'None'}

## Current objective

${data.intent?.goal ?? 'None'}

## Scope

${data.intent?.scope ?? 'None'}

## Product requirements

${list(data.productRequirements ?? [])}

## Technical requirements

${list(data.technicalRequirements ?? [])}

## Open questions

${questionList(data.openQuestions ?? [])}
`;

const factList = (
  facts: readonly PromptExploration['facts'][number][],
): string =>
  facts.length === 0
    ? '- None'
    : facts
        .map(
          (fact) =>
            `- ${fact.fact}\n  - Evidence: ${listInline(fact.evidencePaths)}`,
        )
        .join('\n');

const list = (items: readonly string[]): string =>
  items.length === 0 ? '- None' : items.map((item) => `- ${item}`).join('\n');

const listInline = (items: readonly string[]): string =>
  items.length === 0 ? 'None' : items.join(', ');

const questionList = (questions: readonly PromptOpenQuestion[]): string =>
  questions.length === 0
    ? '- None'
    : questions
        .map(
          (question) =>
            `- ${question.question}\n  - Impact: ${question.impact ?? 'Unspecified'}\n  - Recommendation: ${question.recommendation}\n  - Options:${nestedList(question.options)}`,
        )
        .join('\n');

const nestedList = (items: readonly string[]): string =>
  items.length === 0
    ? ' None'
    : `\n${items.map((item) => `    - ${item}`).join('\n')}`;
