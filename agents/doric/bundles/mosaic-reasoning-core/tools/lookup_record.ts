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
  "collection": z.string().min(1).describe("Nome da coleção."),
  "id": z.string().min(1).describe("Identificador do registro."),
  "fields": z.array(z.string()).optional(),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "collection": {
      "type": "string",
      "description": "Nome da coleção.",
      "minLength": 1
    },
    "id": {
      "type": "string",
      "description": "Identificador do registro.",
      "minLength": 1
    },
    "fields": {
      "type": "array",
      "items": {
        "type": "string"
      }
    }
  },
  "additionalProperties": false,
  "required": [
    "collection",
    "id"
  ]
} as JsonObject;

const tool = {
  name: "lookup_record",
  description: "Consulta um registro simulado por coleção e identificador.",
  schema,
  definition: {
    name: "lookup_record",
    description: "Consulta um registro simulado por coleção e identificador.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
