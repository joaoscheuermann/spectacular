import type {
  JsonValue,
  ToolCallRequest,
  ToolDefinition,
} from 'tools';
import type { z } from 'zod';

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
  readonly structuredOutputs: boolean;
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

export type StructuredOutputSchema = z.ZodObject;
export type StructuredOutputValue<Schema extends StructuredOutputSchema> =
  z.output<Schema>;

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

export type ProviderFinished<Output = JsonValue> = {
  readonly text: string;
  readonly finishReason: FinishReason;
  readonly usage?: UsageMetadata;
  readonly reasoning?: ReasoningMetadata;
  readonly refusal?: string;
  readonly toolCalls: readonly ProviderToolCall[];
  readonly structured?: Output;
};

export type ProviderError = {
  readonly provider: ProviderId;
  readonly code: string;
  readonly message: string;
  readonly status?: number;
  readonly retryable?: boolean;
  readonly diagnostic?: string;
};

export type ProviderStreamEvent<Output = JsonValue> =
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
      readonly finish: ProviderFinished<Output>;
    }
  | {
      readonly type: 'error';
      readonly error: ProviderError;
    };

export type ProviderRequest<
  Output = JsonValue,
  Schema extends StructuredOutputSchema = StructuredOutputSchema,
> = {
  readonly model: string;
  readonly messages: readonly ProviderMessage[];
  readonly tools?: readonly ToolDefinition[];
  readonly schema?: Schema;
  readonly temperature?: number;
  readonly maxOutputTokens?: number;
  readonly flags?: ProviderCallFlags;
  readonly signal?: AbortSignal;
};

/** Provider-neutral completion and streaming contract for agent-core callers. */
export interface LlmProvider {
  readonly metadata: ProviderMetadata;
  readonly capabilities: ProviderCapabilities;

  complete<
    Schema extends StructuredOutputSchema,
    Output = z.output<Schema>,
  >(
    request: ProviderRequest<Output, Schema> & {
      readonly schema: Schema;
    },
  ): Promise<ProviderFinished<Output>>;

  complete<Output = JsonValue>(
    request: ProviderRequest<Output>,
  ): Promise<ProviderFinished<Output>>;

  stream<Schema extends StructuredOutputSchema, Output = z.output<Schema>>(
    request: ProviderRequest<Output, Schema> & {
      readonly schema: Schema;
    },
  ): AsyncIterable<ProviderStreamEvent<Output>>;

  stream<Output = JsonValue>(
    request: ProviderRequest<Output>,
  ): AsyncIterable<ProviderStreamEvent<Output>>;

  models(signal?: AbortSignal): Promise<readonly Model[]>;

  validateModel(model: string, signal?: AbortSignal): Promise<Model>;
}
