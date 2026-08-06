import type { z } from 'zod';
import type { Sandbox } from 'sandbox';

import type { ToolDefinitionSchema } from '../schemas/definition.js';
import type { ToolMetadataSchema } from '../schemas/metadata.js';
import type { JsonValue } from './json.js';

export type ToolSchema = z.ZodObject;

type ToolDefinitionValue = z.output<typeof ToolDefinitionSchema>;
type ToolMetadataValue = z.output<typeof ToolMetadataSchema>;

export type ToolDefinition = {
  readonly [Key in keyof ToolDefinitionValue]: ToolDefinitionValue[Key];
};

export type ToolMetadata = {
  readonly [Key in keyof ToolMetadataValue]: ToolMetadataValue[Key];
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

export type ToolFactoryHandler<Schema extends ToolSchema, Result = unknown> = (
  sandbox: Sandbox,
  payload: z.output<Schema>,
) => Result | Promise<Result>;

export type DefineToolOptions<Schema extends ToolSchema, Result = unknown> = {
  readonly name: string;
  readonly description?: string;
  readonly schema: Schema;
  readonly execute: ToolFactoryHandler<Schema, Result>;
  readonly strict?: boolean;
};

export type ToolFactory<
  Schema extends ToolSchema = ToolSchema,
  Result = unknown,
> = {
  (sandbox: Sandbox): Tool<Schema, Result>;
  readonly name: string;
  readonly description?: string;
  readonly schema: Schema;
  readonly definition: ToolDefinition;
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
