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
  "expression": z.string().min(1).describe("Expressão matemática sem efeitos colaterais."),
  "precision": z.number().int().min(0).max(15).describe("Casas decimais opcionais.").optional(),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "expression": {
      "type": "string",
      "description": "Expressão matemática sem efeitos colaterais.",
      "minLength": 1
    },
    "precision": {
      "type": "integer",
      "description": "Casas decimais opcionais.",
      "minimum": 0,
      "maximum": 15
    }
  },
  "additionalProperties": false,
  "required": [
    "expression"
  ]
} as JsonObject;

const tool = {
  name: "calculator",
  description: "Avalia uma expressão matemática determinística e retorna o resultado.",
  schema,
  definition: {
    name: "calculator",
    description: "Avalia uma expressão matemática determinística e retorna o resultado.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
