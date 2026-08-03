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
  "path": z.string().min(1).describe("Escopo de busca."),
  "query": z.string().min(1).describe("Texto ou expressão a localizar."),
  "isRegex": z.boolean().default(false),
  "caseSensitive": z.boolean().default(false),
  "maxMatches": z.number().int().min(1).max(1000).default(100),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Escopo de busca.",
      "minLength": 1
    },
    "query": {
      "type": "string",
      "description": "Texto ou expressão a localizar.",
      "minLength": 1
    },
    "isRegex": {
      "type": "boolean",
      "default": false
    },
    "caseSensitive": {
      "type": "boolean",
      "default": false
    },
    "maxMatches": {
      "type": "integer",
      "minimum": 1,
      "maximum": 1000,
      "default": 100
    }
  },
  "additionalProperties": false,
  "required": [
    "path",
    "query"
  ]
} as JsonObject;

const tool = {
  name: "search_text",
  description: "Busca texto ou expressão regular em arquivos ou documentos.",
  schema,
  definition: {
    name: "search_text",
    description: "Busca texto ou expressão regular em arquivos ou documentos.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
