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
  "path": z.string().min(1).describe("Caminho de destino."),
  "content": z.string().describe("Conteúdo completo a persistir."),
  "overwrite": z.boolean().describe("Autoriza substituir arquivo existente.").default(false),
  "encoding": z.literal("utf-8").describe("Codificação textual.").optional(),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Caminho de destino.",
      "minLength": 1
    },
    "content": {
      "type": "string",
      "description": "Conteúdo completo a persistir."
    },
    "overwrite": {
      "type": "boolean",
      "description": "Autoriza substituir arquivo existente.",
      "default": false
    },
    "encoding": {
      "type": "string",
      "description": "Codificação textual.",
      "enum": [
        "utf-8"
      ]
    }
  },
  "additionalProperties": false,
  "required": [
    "path",
    "content"
  ]
} as JsonObject;

const tool = {
  name: "write",
  description: "Cria ou substitui um arquivo textual com conteúdo completo.",
  schema,
  definition: {
    name: "write",
    description: "Cria ou substitui um arquivo textual com conteúdo completo.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
