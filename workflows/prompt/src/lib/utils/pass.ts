import { createAgent } from 'agent';
import type { StructuredOutputSchema, StructuredOutputValue } from 'llms';
import { createMessageStorage } from 'messages';
import type { ToolStorage } from 'tool';

import type {
  PromptArtifact,
  PromptWorkflowOptions,
} from '../types/prompt.js';
import { artifactInput } from './prompts.js';

export type PromptPass<Schema extends StructuredOutputSchema> = {
  readonly name: string;
  readonly system: string;
  readonly schema: Schema;
  readonly tools: ToolStorage;
};

export const completePass = async <Schema extends StructuredOutputSchema>(
  artifact: PromptArtifact,
  options: PromptWorkflowOptions,
  pass: PromptPass<Schema>,
): Promise<StructuredOutputValue<Schema>> => {
  const agent = createAgent({
    provider: options.provider,
    tools: pass.tools,
    messages: createMessageStorage(),
    system: pass.system,
    model: options.model,
    ...(options.effort !== undefined ? { effort: options.effort } : {}),
    temperature: 0,
  });

  const response = await agent.complete(artifactInput(artifact.data), {
    signal: options.signal,
    schema: pass.schema,
  });

  if (response.structured === undefined) {
    throw new Error(`${pass.name} did not return structured output.`);
  }

  return response.structured;
};
