import { createAgent, createToolCallStorage } from 'agent';
import type {
  LlmProvider,
  ProviderCallFlags,
  StructuredOutputSchema,
  StructuredOutputValue,
} from 'llms';
import { createMessageStorage } from 'messages';
import { createToolStorage } from 'tool';
import type { MosaicRuntime } from './observability.js';
import type { MosaicStage } from './types/events.js';
import type { MosaicModelProfile } from './types/mosaic-options.js';

type StructuredCompletion<Schema extends StructuredOutputSchema> = {
  readonly provider: LlmProvider;
  readonly profile: MosaicModelProfile;
  readonly system: string;
  readonly input: string;
  readonly schema: Schema;
  readonly flags?: ProviderCallFlags;
  readonly runtime?: MosaicRuntime;
  readonly stage?: MosaicStage;
  readonly nodeId?: string;
  readonly revision?: number;
};

/** Runs one isolated structured operation through the agent's terminal tool. */
export const completeStructured = async <
  Schema extends StructuredOutputSchema,
>({
  provider,
  profile,
  system,
  input,
  schema,
  flags,
  runtime,
  stage = 'run',
  nodeId,
  revision,
}: StructuredCompletion<Schema>): Promise<StructuredOutputValue<Schema>> => {
  const agent = createAgent({
    provider: runtime?.provider(provider, stage, nodeId, revision) ?? provider,
    model: profile.model,
    system,
    flags,
    messages: createMessageStorage(),
    toolCalls: createToolCallStorage(),
    tools: createToolStorage([]),
    effort: profile.effort,
  });
  const response = await agent.complete(input, {
    schema,
    ...(runtime === undefined
      ? {}
      : {
          onStructuredAttempt: (event) =>
            runtime.emit({
              type: 'structured.attempt',
              providerId: provider.metadata.id,
              stage,
              ...(nodeId === undefined ? {} : { nodeId }),
              ...(revision === undefined ? {} : { revision }),
              attempt: event.attempt,
              runtimeAccepted: event.runtimeAccepted,
              feedbackSent: event.feedbackSent,
              ...(event.diagnostic === undefined
                ? {}
                : { diagnostic: event.diagnostic }),
            }),
          onToolCallRepair: (event) =>
            runtime.emit({
              type: 'tool.repair',
              providerId: provider.metadata.id,
              stage,
              ...(nodeId === undefined ? {} : { nodeId }),
              ...(revision === undefined ? {} : { revision }),
              attempt: event.attempt,
              maxAttempts: event.maxAttempts,
            }),
        }),
  });

  return schema.parse(response.structured);
};
