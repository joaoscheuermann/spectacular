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
  "destinationType": z.enum(["channel", "contact"]),
  "destinationId": z.string().min(1),
  "text": z.string().min(1),
  "threadId": z.string().optional(),
  "idempotencyKey": z.string().optional(),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "destinationType": {
      "type": "string",
      "enum": [
        "channel",
        "contact"
      ]
    },
    "destinationId": {
      "type": "string",
      "minLength": 1
    },
    "text": {
      "type": "string",
      "minLength": 1
    },
    "threadId": {
      "type": "string"
    },
    "idempotencyKey": {
      "type": "string"
    }
  },
  "additionalProperties": false,
  "required": [
    "destinationType",
    "destinationId",
    "text"
  ]
} as JsonObject;

const tool = {
  name: "send_message",
  description: "Envia uma mensagem simulada a um canal ou contato e retorna um identificador observável.",
  schema,
  definition: {
    name: "send_message",
    description: "Envia uma mensagem simulada a um canal ou contato e retorna um identificador observável.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
