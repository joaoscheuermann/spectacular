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
  "fixtureId": z.string().min(1).describe("Identificador da fixture."),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "fixtureId": {
      "type": "string",
      "description": "Identificador da fixture.",
      "minLength": 1
    }
  },
  "additionalProperties": false,
  "required": [
    "fixtureId"
  ]
} as JsonObject;

const tool = {
  name: "read_fixture",
  description: "Lê uma fixture opaca do ambiente de testes por identificador conhecido.",
  schema,
  definition: {
    name: "read_fixture",
    description: "Lê uma fixture opaca do ambiente de testes por identificador conhecido.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
