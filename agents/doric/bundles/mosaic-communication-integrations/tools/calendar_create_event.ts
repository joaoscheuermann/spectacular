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
  "title": z.string().min(1),
  "start": z.string().datetime({ offset: true }),
  "end": z.string().datetime({ offset: true }),
  "attendeeIds": z.array(z.string()).optional(),
  "description": z.string().optional(),
  "location": z.string().optional(),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "title": {
      "type": "string",
      "minLength": 1
    },
    "start": {
      "type": "string",
      "format": "date-time"
    },
    "end": {
      "type": "string",
      "format": "date-time"
    },
    "attendeeIds": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "description": {
      "type": "string"
    },
    "location": {
      "type": "string"
    }
  },
  "additionalProperties": false,
  "required": [
    "title",
    "start",
    "end"
  ]
} as JsonObject;

const tool = {
  name: "calendar_create_event",
  description: "Cria um evento simulado com horário, participantes e descrição.",
  schema,
  definition: {
    name: "calendar_create_event",
    description: "Cria um evento simulado com horário, participantes e descrição.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
