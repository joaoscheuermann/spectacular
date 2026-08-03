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
  "records": z.array(z.record(z.string(), z.unknown())),
  "sortBy": z.array(z.object({
  "field": z.string(),
  "direction": z.enum(["asc", "desc"]),
  "nulls": z.enum(["first", "last"]).optional(),
}).strict()).min(1),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "records": {
      "type": "array",
      "items": {
        "type": "object"
      }
    },
    "sortBy": {
      "type": "array",
      "items": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
          "field": {
            "type": "string"
          },
          "direction": {
            "type": "string",
            "enum": [
              "asc",
              "desc"
            ]
          },
          "nulls": {
            "type": "string",
            "enum": [
              "first",
              "last"
            ]
          }
        },
        "additionalProperties": false,
        "required": [
          "field",
          "direction"
        ]
      },
      "minItems": 1
    }
  },
  "additionalProperties": false,
  "required": [
    "records",
    "sortBy"
  ]
} as JsonObject;

const tool = {
  name: "sort_records",
  description: "Ordena registros estruturados por uma ou mais chaves.",
  schema,
  definition: {
    name: "sort_records",
    description: "Ordena registros estruturados por uma ou mais chaves.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
