import type { z } from 'zod';

import type { JsonObject, JsonValue } from './json.js';

export type ToolSchema = z.ZodObject;

export type ToolDefinition = {
  readonly name: string;
  readonly description?: string;
  readonly inputSchema: JsonObject;
  readonly strict?: boolean;
};

export type ToolCallRequest = {
  readonly id: string;
  readonly name: string;
  readonly arguments: string;
  readonly index?: number;
};

export type ToolCall = {
  readonly id: string;
  readonly name: string;
  readonly payload: JsonValue;
  readonly index?: number;
};

export type ToolTurn = {
  readonly toolCalls?: readonly ToolCallRequest[];
};

export type ToolHandler<Schema extends ToolSchema, Result = unknown> = (
  payload: z.output<Schema>,
) => Result | Promise<Result>;

export type Tool<Schema extends ToolSchema = ToolSchema, Result = unknown> = {
  readonly name: string;
  readonly description?: string;
  readonly schema: Schema;
  readonly definition: ToolDefinition;
  readonly execute: ToolHandler<Schema, Result>;
};

export type CreateToolOptions<Schema extends ToolSchema, Result = unknown> = {
  readonly name: string;
  readonly description?: string;
  readonly schema: Schema;
  readonly execute: ToolHandler<Schema, Result>;
  readonly strict?: boolean;
};

export type ToolStorage = {
  definitions(): readonly ToolDefinition[];
  calls(turn: ToolTurn): readonly ToolCall[];
  get(name: string): Tool | undefined;
  execute(call: ToolCall | ToolCallRequest): Promise<unknown>;
};

export type ToolErrorCode =
  | 'duplicate_tool'
  | 'handler_failed'
  | 'invalid_json'
  | 'invalid_payload'
  | 'invalid_schema'
  | 'unknown_tool';

export type ToolIssue = {
  readonly path: string;
  readonly message: string;
};

export type ToolError = {
  readonly code: ToolErrorCode;
  readonly message: string;
  readonly toolName?: string;
  readonly callId?: string;
  readonly diagnostic?: string;
  readonly issues?: readonly ToolIssue[];
};
