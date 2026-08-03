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
  "rootPath": z.string().min(1),
  "depth": z.number().int().min(0).max(20).default(3),
  "includeHidden": z.boolean().default(false),
  "maxEntries": z.number().int().min(1).max(5000).default(500),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "rootPath": {
      "type": "string",
      "minLength": 1
    },
    "depth": {
      "type": "integer",
      "minimum": 0,
      "maximum": 20,
      "default": 3
    },
    "includeHidden": {
      "type": "boolean",
      "default": false
    },
    "maxEntries": {
      "type": "integer",
      "minimum": 1,
      "maximum": 5000,
      "default": 500
    }
  },
  "additionalProperties": false,
  "required": [
    "rootPath"
  ]
} as JsonObject;

const tool = {
  name: "repository_tree",
  description: "Retorna a árvore de arquivos de um repositório com profundidade controlada.",
  schema,
  definition: {
    name: "repository_tree",
    description: "Retorna a árvore de arquivos de um repositório com profundidade controlada.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
