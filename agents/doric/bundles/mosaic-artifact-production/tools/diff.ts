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
  "repositoryPath": z.string().min(1),
  "baseRef": z.string().min(1),
  "targetRef": z.string().min(1),
  "path": z.string().optional(),
  "contextLines": z.number().int().min(0).max(20).default(3),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "repositoryPath": {
      "type": "string",
      "minLength": 1
    },
    "baseRef": {
      "type": "string",
      "minLength": 1
    },
    "targetRef": {
      "type": "string",
      "minLength": 1
    },
    "path": {
      "type": "string"
    },
    "contextLines": {
      "type": "integer",
      "minimum": 0,
      "maximum": 20,
      "default": 3
    }
  },
  "additionalProperties": false,
  "required": [
    "repositoryPath",
    "baseRef",
    "targetRef"
  ]
} as JsonObject;

const tool = {
  name: "diff",
  description: "Obtém diferenças textuais entre duas referências ou para um caminho específico.",
  schema,
  definition: {
    name: "diff",
    description: "Obtém diferenças textuais entre duas referências ou para um caminho específico.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
