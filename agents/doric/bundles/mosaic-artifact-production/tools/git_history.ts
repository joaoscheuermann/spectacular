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
  "repositoryPath": z.string().min(1),
  "path": z.string().optional(),
  "sinceRef": z.string().optional(),
  "untilRef": z.string().optional(),
  "limit": z.number().int().min(1).max(500).default(50),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "repositoryPath": {
      "type": "string",
      "minLength": 1
    },
    "path": {
      "type": "string"
    },
    "sinceRef": {
      "type": "string"
    },
    "untilRef": {
      "type": "string"
    },
    "limit": {
      "type": "integer",
      "minimum": 1,
      "maximum": 500,
      "default": 50
    }
  },
  "additionalProperties": false,
  "required": [
    "repositoryPath"
  ]
} as JsonObject;

const tool = {
  name: "git_history",
  description: "Consulta histórico Git simulado para um repositório, caminho ou intervalo.",
  schema,
  definition: {
    name: "git_history",
    description: "Consulta histórico Git simulado para um repositório, caminho ou intervalo.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
