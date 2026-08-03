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
  "path": z.string().min(1).describe("Caminho ou referência do artefato."),
  "startLine": z.number().int().min(1).describe("Linha inicial inclusiva, indexada em 1.").optional(),
  "endLine": z.number().int().min(1).describe("Linha final inclusiva, indexada em 1.").optional(),
  "maxChars": z.number().int().min(1).max(1000000).describe("Limite de caracteres retornados.").optional(),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Caminho ou referência do artefato.",
      "minLength": 1
    },
    "startLine": {
      "type": "integer",
      "description": "Linha inicial inclusiva, indexada em 1.",
      "minimum": 1
    },
    "endLine": {
      "type": "integer",
      "description": "Linha final inclusiva, indexada em 1.",
      "minimum": 1
    },
    "maxChars": {
      "type": "integer",
      "description": "Limite de caracteres retornados.",
      "minimum": 1,
      "maximum": 1000000
    }
  },
  "additionalProperties": false,
  "required": [
    "path"
  ]
} as JsonObject;

const tool = {
  name: "read",
  description: "Lê conteúdo textual de um arquivo ou documento por caminho ou referência.",
  schema,
  definition: {
    name: "read",
    description: "Lê conteúdo textual de um arquivo ou documento por caminho ou referência.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
