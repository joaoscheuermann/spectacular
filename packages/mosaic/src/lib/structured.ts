import { createAgent } from 'agent';
import type {
  LlmProvider,
  ProviderCallFlags,
  StructuredOutputSchema,
  StructuredOutputValue,
} from 'llms';
import { createMessageStorage } from 'messages';
import { createToolStorage } from 'tool';

type StructuredCompletion<Schema extends StructuredOutputSchema> = {
  readonly provider: LlmProvider;
  readonly model: string;
  readonly system: string;
  readonly input: string;
  readonly schema: Schema;
  readonly flags?: ProviderCallFlags;
};

/** Runs one isolated structured operation through the agent's terminal tool. */
export const completeStructured = async <
  Schema extends StructuredOutputSchema,
>({
  provider,
  model,
  system,
  input,
  schema,
  flags,
}: StructuredCompletion<Schema>): Promise<StructuredOutputValue<Schema>> => {
  const agent = createAgent({
    provider,
    model,
    system,
    flags,
    messages: createMessageStorage(),
    tools: createToolStorage([]),
  });
  const response = await agent.complete(input, { schema });

  return schema.parse(response.structured);
};
