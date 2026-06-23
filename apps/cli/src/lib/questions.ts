import type { AgentExecutionEvent } from '@a2a-js/sdk/server';

export type PromptQuestion = {
  readonly id: string;
  readonly question: string;
  readonly options: readonly {
    readonly title: string;
    readonly value: string;
  }[];
};

export type Prompt = (
  questions: readonly {
    readonly type: 'list';
    readonly name: string;
    readonly message: string;
    readonly choices: readonly { readonly name: string; readonly value: string }[];
  }[],
) => Promise<Record<string, string>>;

export const openQuestionsFromEvent = (
  event: AgentExecutionEvent,
): readonly PromptQuestion[] => {
  if (
    event.kind !== 'status-update' ||
    event.status.state !== 'input-required' ||
    event.status.message === undefined
  ) {
    return [];
  }

  return event.status.message.parts.flatMap((part) => {
    if (
      part.kind !== 'data' ||
      !isRecord(part.data) ||
      part.data['kind'] !== 'prompt-open-questions' ||
      !Array.isArray(part.data['questions'])
    ) {
      return [];
    }

    return part.data['questions'].filter(isPromptQuestion);
  });
};

export const askOpenQuestions = async (
  questions: readonly PromptQuestion[],
  prompt: Prompt,
): Promise<string> => {
  const answers = await prompt(
    questions.map((question) => ({
      type: 'list',
      name: question.id,
      message: question.question,
      choices: question.options.map((option) => ({
        name: option.title,
        value: option.value,
      })),
    })),
  );

  return questions
    .map((question) => `- ${question.question}: ${answers[question.id] ?? ''}`)
    .join('\n');
};

const isPromptQuestion = (value: unknown): value is PromptQuestion => {
  if (!isRecord(value)) {
    return false;
  }

  return (
    typeof value['id'] === 'string' &&
    typeof value['question'] === 'string' &&
    Array.isArray(value['options']) &&
    value['options'].every(isPromptOption)
  );
};

const isPromptOption = (
  value: unknown,
): value is { readonly title: string; readonly value: string } =>
  isRecord(value) &&
  typeof value['title'] === 'string' &&
  typeof value['value'] === 'string';

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null;
