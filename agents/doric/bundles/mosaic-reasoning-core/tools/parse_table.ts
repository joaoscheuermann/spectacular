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
  "path": z.string().describe("Documento que contém a tabela.").optional(),
  "content": z.string().describe("Conteúdo textual alternativo ao path.").optional(),
  "tableIndex": z.number().int().min(0).describe("Índice da tabela, começando em 0.").default(0),
  "headerRow": z.number().int().min(0).describe("Linha de cabeçalho relativa à tabela.").default(0),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Documento que contém a tabela."
    },
    "content": {
      "type": "string",
      "description": "Conteúdo textual alternativo ao path."
    },
    "tableIndex": {
      "type": "integer",
      "description": "Índice da tabela, começando em 0.",
      "minimum": 0,
      "default": 0
    },
    "headerRow": {
      "type": "integer",
      "description": "Linha de cabeçalho relativa à tabela.",
      "minimum": 0,
      "default": 0
    }
  },
  "additionalProperties": false
} as JsonObject;

const tool = {
  name: "parse_table",
  description: "Extrai uma tabela de conteúdo textual ou de um documento para linhas e colunas estruturadas.",
  schema,
  definition: {
    name: "parse_table",
    description: "Extrai uma tabela de conteúdo textual ou de um documento para linhas e colunas estruturadas.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
