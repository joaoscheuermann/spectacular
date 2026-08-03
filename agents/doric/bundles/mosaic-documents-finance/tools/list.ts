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
  "path": z.string().min(1).describe("Diretório ou namespace inicial."),
  "pattern": z.string().describe("Padrão opcional de nome ou glob.").optional(),
  "recursive": z.boolean().describe("Inclui descendentes.").default(false),
  "limit": z.number().int().min(1).max(1000).describe("Máximo de entradas retornadas.").default(100),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Diretório ou namespace inicial.",
      "minLength": 1
    },
    "pattern": {
      "type": "string",
      "description": "Padrão opcional de nome ou glob."
    },
    "recursive": {
      "type": "boolean",
      "description": "Inclui descendentes.",
      "default": false
    },
    "limit": {
      "type": "integer",
      "description": "Máximo de entradas retornadas.",
      "minimum": 1,
      "maximum": 1000,
      "default": 100
    }
  },
  "additionalProperties": false,
  "required": [
    "path"
  ]
} as JsonObject;

const tool = {
  name: "list",
  description: "Lista entradas de um diretório ou namespace de documentos simulado.",
  schema,
  definition: {
    name: "list",
    description: "Lista entradas de um diretório ou namespace de documentos simulado.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
