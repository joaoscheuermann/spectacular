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
  "kind": z.enum(["email", "direct-message"]),
  "recipientIds": z.array(z.string()).min(1),
  "subject": z.string().optional(),
  "body": z.string().min(1),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "kind": {
      "type": "string",
      "enum": [
        "email",
        "direct-message"
      ]
    },
    "recipientIds": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "minItems": 1
    },
    "subject": {
      "type": "string"
    },
    "body": {
      "type": "string",
      "minLength": 1
    }
  },
  "additionalProperties": false,
  "required": [
    "kind",
    "recipientIds",
    "body"
  ]
} as JsonObject;

const tool = {
  name: "create_draft",
  description: "Cria um rascunho de email ou mensagem privada sem efetuar envio.",
  schema,
  definition: {
    name: "create_draft",
    description: "Cria um rascunho de email ou mensagem privada sem efetuar envio.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
