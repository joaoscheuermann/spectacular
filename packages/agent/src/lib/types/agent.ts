import type {
  FinishReason,
  JsonValue,
  LlmProvider,
  ProviderCallFlags,
  ProviderFinished,
  ProviderStreamEvent,
  ReasoningEffort,
  ReasoningMetadata,
  StructuredOutputSchema,
  StructuredOutputValue,
  UsageMetadata,
} from 'llms';
import type { MessageStorage } from 'messages';
import type { ToolCall, ToolStorage } from 'tool';

export type AgentOptions = {
  readonly provider: LlmProvider;
  readonly tools: ToolStorage;
  readonly messages: MessageStorage;
  readonly system: string;
  readonly model: string;
  readonly effort?: ReasoningEffort;
  readonly flags?: ProviderCallFlags;
  readonly temperature?: number;
  readonly maxOutputTokens?: number;
};

export type AgentRunOptions<
  Output = JsonValue,
  Schema extends StructuredOutputSchema = StructuredOutputSchema,
> = {
  readonly maxTurns?: number;
  readonly signal?: AbortSignal;
  readonly schema?: Schema;
};

export type Agent = {
  readonly complete: {
    <
      Schema extends StructuredOutputSchema,
      Output = StructuredOutputValue<Schema>,
    >(
      input: string,
      options: AgentRunOptions<Output, Schema> & {
        readonly schema: Schema;
      },
    ): Promise<AgentResponse<Output>>;
    <Output = JsonValue>(
      input: string,
      options?: AgentRunOptions<Output>,
    ): Promise<AgentResponse<Output>>;
  };
  readonly stream: {
    <
      Schema extends StructuredOutputSchema,
      Output = StructuredOutputValue<Schema>,
    >(
      input: string,
      options: AgentRunOptions<Output, Schema> & {
        readonly schema: Schema;
      },
    ): AsyncIterable<AgentEvent<Output>>;
    <Output = JsonValue>(
      input: string,
      options?: AgentRunOptions<Output>,
    ): AsyncIterable<AgentEvent<Output>>;
  };
};

export type AgentComplete = Agent['complete'];

export type AgentStream = Agent['stream'];

export type AgentResponse<Output = JsonValue> = {
  readonly text: string;
  readonly finishReason: FinishReason;
  readonly usage?: UsageMetadata;
  readonly reasoning?: ReasoningMetadata;
  readonly refusal?: string;
  readonly structured?: Output;
  readonly finish: ProviderFinished<Output>;
};

export type AgentEvent<Output = JsonValue> =
  | ProviderStreamEvent<Output>
  | {
      readonly type: 'agent.started';
      readonly model: string;
      readonly input: string;
    }
  | {
      readonly type: 'agent.finished';
      readonly response: AgentResponse<Output>;
    }
  | { readonly type: 'tool.started'; readonly call: ToolCall }
  | {
      readonly type: 'tool.finished';
      readonly call: ToolCall;
      readonly result: unknown;
      readonly content: string;
    }
  | {
      readonly type: 'tool.failed';
      readonly call: ToolCall;
      readonly error: unknown;
    };

export type AgentErrorCode =
  | 'concurrent_run'
  | 'invalid_structured_output'
  | 'missing_provider_finish'
  | 'turn_limit_exceeded'
  | 'tool_result_serialization_failed';

export type AgentError = {
  readonly code: AgentErrorCode;
  readonly message: string;
  readonly diagnostic?: string;
};
