import type {
  FinishReason,
  LlmProvider,
  ProviderCallFlags,
  ProviderFinished,
  ProviderStreamEvent,
  ReasoningMetadata,
  UsageMetadata,
} from 'llms';
import type { MessageStorage } from 'messages';
import type { ToolCall, ToolStorage } from 'tools';

export type AgentOptions = {
  readonly provider: LlmProvider;
  readonly tools: ToolStorage;
  readonly messages: MessageStorage;
  readonly system: string;
  readonly model: string;
  readonly flags?: ProviderCallFlags;
  readonly temperature?: number;
  readonly maxOutputTokens?: number;
};

export type AgentRunOptions = {
  readonly signal?: AbortSignal;
};

export type Agent = {
  readonly complete: (
    input: string,
    options?: AgentRunOptions,
  ) => Promise<AgentResponse>;
  readonly stream: (
    input: string,
    options?: AgentRunOptions,
  ) => AsyncIterable<AgentEvent>;
};

export type AgentResponse = {
  readonly text: string;
  readonly finishReason: FinishReason;
  readonly usage?: UsageMetadata;
  readonly reasoning?: ReasoningMetadata;
  readonly refusal?: string;
  readonly finish: ProviderFinished;
};

export type AgentEvent =
  | ProviderStreamEvent
  | {
      readonly type: 'agent.started';
      readonly model: string;
      readonly input: string;
    }
  | { readonly type: 'agent.finished'; readonly response: AgentResponse }
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
  | 'missing_provider_finish'
  | 'tool_result_serialization_failed';

export type AgentError = {
  readonly code: AgentErrorCode;
  readonly message: string;
  readonly diagnostic?: string;
};
