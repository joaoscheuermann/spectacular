import type {
  ToolCallRequest,
  ToolDefinition,
} from 'tools';

export type { JsonArray, JsonObject, JsonPrimitive, JsonValue } from 'tools';

export type ProviderId = 'openai' | 'openrouter' | string;

export type ProviderMetadata = {
  readonly id: ProviderId;
  readonly name: string;
  readonly baseUrl: string;
};

export type ProviderCapabilities = {
  readonly streaming: boolean;
  readonly tools: boolean;
  readonly reasoning: boolean;
  readonly modelListing: boolean;
  readonly oauth: boolean;
  readonly serviceTier: boolean;
};

export type Model = {
  readonly id: string;
  readonly name?: string;
  readonly contextWindow?: number;
  readonly provider?: string;
  readonly raw?: unknown;
};

export type ProviderContentPart =
  | {
      readonly type: 'text';
      readonly text: string;
    }
  | {
      readonly type: 'image';
      readonly imageUrl: string;
    };

export type ProviderMessage = {
  readonly role: 'system' | 'user' | 'assistant' | 'tool';
  readonly content?: string | readonly ProviderContentPart[];
  readonly name?: string;
  readonly toolCallId?: string;
  readonly toolCalls?: readonly ProviderToolCall[];
};

export type ToolManifest = ToolDefinition;

export type ProviderToolCall = ToolCallRequest;

export type ReasoningRequest = {
  readonly effort?: 'minimal' | 'low' | 'medium' | 'high';
  readonly summary?: 'auto' | 'concise' | 'detailed';
};

export type ProviderCallFlags = {
  readonly reasoning?: boolean | ReasoningRequest;
  readonly serviceTier?: 'auto' | 'default' | 'priority';
  readonly includeUsage?: boolean;
};

export type FinishReason =
  | 'stop'
  | 'length'
  | 'tool_calls'
  | 'content_filter'
  | 'error'
  | 'cancelled'
  | 'unknown';

export type UsageMetadata = {
  readonly inputTokens?: number;
  readonly outputTokens?: number;
  readonly totalTokens?: number;
  readonly reasoningTokens?: number;
  readonly cachedInputTokens?: number;
};

export type ReasoningMetadata = {
  readonly text?: string;
  readonly effort?: ReasoningRequest['effort'];
  readonly summary?: string;
};

export type ProviderFinished = {
  readonly text: string;
  readonly finishReason: FinishReason;
  readonly usage?: UsageMetadata;
  readonly reasoning?: ReasoningMetadata;
  readonly refusal?: string;
  readonly toolCalls: readonly ProviderToolCall[];
};

export type ProviderError = {
  readonly provider: ProviderId;
  readonly code: string;
  readonly message: string;
  readonly status?: number;
  readonly retryable?: boolean;
  readonly diagnostic?: string;
};

export type ProviderStreamEvent =
  | {
      readonly type: 'response.started';
      readonly provider: ProviderId;
      readonly model: string;
    }
  | {
      readonly type: 'text.delta';
      readonly delta: string;
    }
  | {
      readonly type: 'reasoning.delta';
      readonly delta: string;
    }
  | {
      readonly type: 'refusal.delta';
      readonly delta: string;
    }
  | {
      readonly type: 'tool_call.delta';
      readonly index: number;
      readonly id?: string;
      readonly name?: string;
      readonly argumentsDelta?: string;
    }
  | {
      readonly type: 'tool_call.done';
      readonly call: ProviderToolCall;
    }
  | {
      readonly type: 'usage';
      readonly usage: UsageMetadata;
    }
  | {
      readonly type: 'response.finished';
      readonly finish: ProviderFinished;
    }
  | {
      readonly type: 'error';
      readonly error: ProviderError;
    };

export type ProviderRequest = {
  readonly model: string;
  readonly messages: readonly ProviderMessage[];
  readonly tools?: readonly ToolDefinition[];
  readonly temperature?: number;
  readonly maxOutputTokens?: number;
  readonly flags?: ProviderCallFlags;
  readonly signal?: AbortSignal;
};

/** Provider-neutral completion and streaming contract for agent-core callers. */
export interface LlmProvider {
  readonly metadata: ProviderMetadata;
  readonly capabilities: ProviderCapabilities;

  complete(request: ProviderRequest): Promise<ProviderFinished>;

  stream(request: ProviderRequest): AsyncIterable<ProviderStreamEvent>;

  models(signal?: AbortSignal): Promise<readonly Model[]>;

  validateModel(model: string, signal?: AbortSignal): Promise<Model>;
}
