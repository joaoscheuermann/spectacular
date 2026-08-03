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
  "patch": z.string().min(1),
  "dryRun": z.boolean().default(false),
}).strict();

const inputSchema = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "repositoryPath": {
      "type": "string",
      "minLength": 1
    },
    "patch": {
      "type": "string",
      "minLength": 1
    },
    "dryRun": {
      "type": "boolean",
      "default": false
    }
  },
  "additionalProperties": false,
  "required": [
    "repositoryPath",
    "patch"
  ]
} as JsonObject;

const tool = {
  name: "apply_patch",
  description: "Aplica um patch textual a um ou mais arquivos em um workspace simulado.",
  schema,
  definition: {
    name: "apply_patch",
    description: "Aplica um patch textual a um ou mais arquivos em um workspace simulado.",
    inputSchema,
    strict: true,
  },
  execute: async (_payload: z.output<typeof schema>): Promise<void> => {},
} satisfies Tool<typeof schema, void>;

export default tool;
