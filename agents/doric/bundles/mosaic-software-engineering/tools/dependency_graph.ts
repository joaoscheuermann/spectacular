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
  "entryPoints": z.array(z.string()).min(1),
  "direction": z.enum(["upstream", "downstream", "both"]),
  "maxDepth": z.number().int().min(1).max(20).default(5),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "repositoryPath": {
      "type": "string",
      "minLength": 1
    },
    "entryPoints": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "minItems": 1
    },
    "direction": {
      "type": "string",
      "enum": [
        "upstream",
        "downstream",
        "both"
      ]
    },
    "maxDepth": {
      "type": "integer",
      "minimum": 1,
      "maximum": 20,
      "default": 5
    }
  },
  "additionalProperties": false,
  "required": [
    "repositoryPath",
    "entryPoints",
    "direction"
  ]
} as JsonObject;

const tool = {
  name: "dependency_graph",
  description: "Constrói grafo de dependências entre módulos ou pacotes a partir de pontos de entrada.",
  schema,
  definition: {
    name: "dependency_graph",
    description: "Constrói grafo de dependências entre módulos ou pacotes a partir de pontos de entrada.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
