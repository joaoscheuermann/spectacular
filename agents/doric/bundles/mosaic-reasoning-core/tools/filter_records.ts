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
  "predicates": z.array(z.object({
  "field": z.string(),
  "operator": z.enum(["eq", "neq", "gt", "gte", "lt", "lte", "contains", "in", "exists"]),
  "value": z.unknown().optional(),
}).strict()).min(1),
  "mode": z.enum(["all", "any"]),
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
    "predicates": {
      "type": "array",
      "items": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
          "field": {
            "type": "string"
          },
          "operator": {
            "type": "string",
            "enum": [
              "eq",
              "neq",
              "gt",
              "gte",
              "lt",
              "lte",
              "contains",
              "in",
              "exists"
            ]
          },
          "value": {}
        },
        "additionalProperties": false,
        "required": [
          "field",
          "operator"
        ]
      },
      "minItems": 1
    },
    "mode": {
      "type": "string",
      "enum": [
        "all",
        "any"
      ]
    }
  },
  "additionalProperties": false,
  "required": [
    "records",
    "predicates",
    "mode"
  ]
} as JsonObject;

const tool = {
  name: "filter_records",
  description: "Filtra registros estruturados usando predicados declarativos simples.",
  schema,
  definition: {
    name: "filter_records",
    description: "Filtra registros estruturados usando predicados declarativos simples.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
