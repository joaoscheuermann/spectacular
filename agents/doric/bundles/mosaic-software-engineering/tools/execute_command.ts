import { z } from "zod";

type JsonPrimitive = string | number | boolean | null;
type JsonValue = JsonPrimitive | JsonObject | readonly JsonValue[];
type JsonObject = {
  readonly [key: string]: JsonValue;
};

type ToolSchema = z.ZodObject;

type ToolDefinition = {
  readonly name: string;
  readonly description?: string;
  readonly inputSchema: JsonObject;
  readonly strict?: boolean;
};

type ToolHandler<Schema extends ToolSchema, Result = unknown> = (
  payload: z.output<Schema>,
) => Result | Promise<Result>;

type Tool<Schema extends ToolSchema = ToolSchema, Result = unknown> = {
  readonly name: string;
  readonly description?: string;
  readonly schema: Schema;
  readonly definition: ToolDefinition;
  readonly execute: ToolHandler<Schema, Result>;
};

const schema = z.object({
  "command": z.string().min(1),
  "cwd": z.string().optional(),
  "timeoutMs": z.number().int().min(100).max(600000).default(60000),
  "env": z.record(z.string(), z.string()).optional(),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "command": {
      "type": "string",
      "minLength": 1
    },
    "cwd": {
      "type": "string"
    },
    "timeoutMs": {
      "type": "integer",
      "minimum": 100,
      "maximum": 600000,
      "default": 60000
    },
    "env": {
      "type": "object",
      "additionalProperties": {
        "type": "string"
      }
    }
  },
  "additionalProperties": false,
  "required": [
    "command"
  ]
} as JsonObject;

const tool = {
  name: "execute_command",
  description: "Executa um comando permitido em ambiente simulado e retorna stdout, stderr e código de saída.",
  schema,
  definition: {
    name: "execute_command",
    description: "Executa um comando permitido em ambiente simulado e retorna stdout, stderr e código de saída.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
