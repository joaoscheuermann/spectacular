import { createAgent } from 'agent';
import type { LlmProvider, ReasoningEffort } from 'llms';
import { createMessageStorage } from 'messages';
import { createToolStorage } from 'tools';
import type * as z from 'zod';

export type CompletionConfig = {
  readonly effort?: ReasoningEffort;
  readonly model: string;
  readonly provider: LlmProvider;
  readonly signal?: AbortSignal;
};

export type CompletionInput<Schema extends z.ZodType> = CompletionConfig & {
  readonly input: string;
  readonly maxOutputTokens: number;
  readonly schema: Schema;
  readonly stage: string;
  readonly system: string;
};

/** Runs one structured, tool-free completion and validates its output. */
export const complete = async <Schema extends z.ZodType>(
  input: CompletionInput<Schema>,
): Promise<z.output<Schema>> => {
  const response = await createAgent({
    provider: input.provider,
    tools: createToolStorage([]),
    messages: createMessageStorage(),
    system: input.system,
    model: input.model,
    effort: input.effort,
    temperature: 0,
    maxOutputTokens: input.maxOutputTokens,
  }).complete(input.input, { schema: input.schema, signal: input.signal });
  const result = input.schema.safeParse(response.structured);

  if (!result.success) {
    throw new Error(`${input.stage} did not return valid structured output`);
  }

  return result.data;
};
