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
  "query": z.string().min(1),
  "workspace": z.string().optional(),
  "visibility": z.enum(["public", "private", "any"]).optional(),
  "limit": z.number().int().min(1).max(100).default(20),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "minLength": 1
    },
    "workspace": {
      "type": "string"
    },
    "visibility": {
      "type": "string",
      "enum": [
        "public",
        "private",
        "any"
      ]
    },
    "limit": {
      "type": "integer",
      "minimum": 1,
      "maximum": 100,
      "default": 20
    }
  },
  "additionalProperties": false,
  "required": [
    "query"
  ]
} as JsonObject;

const tool = {
  name: "list_channels",
  description: "Busca canais simulados por nome, propósito ou workspace.",
  schema,
  definition: {
    name: "list_channels",
    description: "Busca canais simulados por nome, propósito ou workspace.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
