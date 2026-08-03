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
  "start": z.string().datetime({ offset: true }),
  "end": z.string().datetime({ offset: true }),
  "query": z.string().optional(),
  "attendeeId": z.string().optional(),
  "limit": z.number().int().min(1).max(100).default(20),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "start": {
      "type": "string",
      "format": "date-time"
    },
    "end": {
      "type": "string",
      "format": "date-time"
    },
    "query": {
      "type": "string"
    },
    "attendeeId": {
      "type": "string"
    },
    "limit": {
      "type": "integer",
      "minimum": 1,
      "maximum": 100,
      "default": 20
    }
  },
  "additionalProperties": false,
  "required": [
    "start",
    "end"
  ]
} as JsonObject;

const tool = {
  name: "calendar_list_events",
  description: "Lista eventos simulados em um intervalo para confirmar contexto, participantes e título.",
  schema,
  definition: {
    name: "calendar_list_events",
    description: "Lista eventos simulados em um intervalo para confirmar contexto, participantes e título.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
