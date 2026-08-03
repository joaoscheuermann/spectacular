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
  "path": z.string().min(1).describe("Caminho do arquivo."),
  "content": z.string().describe("Bloco a acrescentar."),
  "separator": z.string().describe("Separador inserido antes do bloco quando necessário.").optional(),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Caminho do arquivo.",
      "minLength": 1
    },
    "content": {
      "type": "string",
      "description": "Bloco a acrescentar."
    },
    "separator": {
      "type": "string",
      "description": "Separador inserido antes do bloco quando necessário."
    }
  },
  "additionalProperties": false,
  "required": [
    "path",
    "content"
  ]
} as JsonObject;

const tool = {
  name: "append",
  description: "Acrescenta conteúdo textual ao final de um arquivo sem substituir o conteúdo anterior.",
  schema,
  definition: {
    name: "append",
    description: "Acrescenta conteúdo textual ao final de um arquivo sem substituir o conteúdo anterior.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
