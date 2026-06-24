import type { AgentExecutionEvent } from '@a2a-js/sdk/server';
import { type Choice, type PromptKit, type PromptTask } from 'prompt-kit';

export type PromptQuestion = {
  readonly id: string;
  readonly question: string;
  readonly options: readonly {
    readonly title: string;
    readonly value: string;
  }[];
};

export type Prompt = Pick<PromptKit, 'queue' | 'select'>;

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
  const tasks = questions.map(
    (question): PromptTask<string> =>
      () =>
        prompt.select(
          question.question,
          '',
          question.options.map(
            (option): Choice<string> => ({
              key: option.title,
              value: option.value,
            }),
          ),
        ),
  );
  const answers = await prompt.queue(tasks);

  return questions
    .map((question, index) => `- ${question.question}: ${answers[index] ?? ''}`)
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
