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
  "target": z.string().optional(),
  "filter": z.string().optional(),
  "timeoutMs": z.number().int().min(100).max(600000).default(120000),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "repositoryPath": {
      "type": "string",
      "minLength": 1
    },
    "target": {
      "type": "string"
    },
    "filter": {
      "type": "string"
    },
    "timeoutMs": {
      "type": "integer",
      "minimum": 100,
      "maximum": 600000,
      "default": 120000
    }
  },
  "additionalProperties": false,
  "required": [
    "repositoryPath"
  ]
} as JsonObject;

const tool = {
  name: "run_tests",
  description: "Executa testes em um projeto simulado com alvo e filtro opcionais.",
  schema,
  definition: {
    name: "run_tests",
    description: "Executa testes em um projeto simulado com alvo e filtro opcionais.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
