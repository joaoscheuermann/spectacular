import { z } from 'zod';

import { ToolErrorObject } from './classes/tool-error.js';
import type { JsonObject } from './types/json.js';
import type {
  CreateToolOptions,
  Tool,
  ToolCall,
  ToolCallRequest,
  ToolDefinition,
  ToolIssue,
  ToolSchema,
  ToolStorage,
  ToolTurn,
} from './types/tool.js';
import { asJsonObject, excerpt, isJsonValue } from './utils/json.js';

/** Creates a typed tool and emits its provider-neutral JSON Schema definition. */
export const createTool = <Schema extends ToolSchema, Result = unknown>(
  options: CreateToolOptions<Schema, Result>,
): Tool<Schema, Result> => {
  if (!(options.schema instanceof z.ZodObject)) {
    throw new ToolErrorObject({
      code: 'invalid_schema',
      toolName: options.name,
      message: `Tool schema must be a Zod object: ${options.name}`,
    });
  }

  const definition: ToolDefinition = {
    name: options.name,
    description: options.description,
    inputSchema: jsonSchema(options.name, options.schema),
    strict: options.strict ?? true,
  };

  return {
    name: options.name,
    description: options.description,
    schema: options.schema,
    definition,
    execute: options.execute,
  };
};

/** Registers tools and provides neutral definitions, call parsing, lookup, and execution. */
export const createToolStorage = (
  tools: readonly Tool[],
): ToolStorage => {
  const entries = tools.map((tool) => [tool.name, tool] as const);
  const names = new Set<string>();
  const duplicate = entries.find(([name]) => {
    if (names.has(name)) {
      return true;
    }

    names.add(name);
    return false;
  });

  if (duplicate !== undefined) {
    throw new ToolErrorObject({
      code: 'duplicate_tool',
      toolName: duplicate[0],
      message: `Tool names must be unique: ${duplicate[0]}`,
    });
  }

  const byName = new Map(entries);

  return {
    definitions: () => tools.map((tool) => tool.definition),
    calls: (turn: ToolTurn) => (turn.toolCalls ?? []).map(parseCall),
    get: (name: string) => byName.get(name),
    execute: async (call: ToolCall | ToolCallRequest): Promise<unknown> => {
      const parsed = normalizeCall(call);
      const tool = byName.get(parsed.name);

      if (tool === undefined) {
        throw new ToolErrorObject({
          code: 'unknown_tool',
          toolName: parsed.name,
          callId: parsed.id,
          message: `Unknown tool requested: ${parsed.name}`,
        });
      }

      const payload = validatePayload(tool, parsed);

      try {
        return await tool.execute(payload);
      } catch (cause) {
        throw new ToolErrorObject(
          {
            code: 'handler_failed',
            toolName: parsed.name,
            callId: parsed.id,
            message: `Tool handler failed: ${parsed.name}`,
          },
          { cause },
        );
      }
    },
  };
};

const jsonSchema = (name: string, schema: ToolSchema): JsonObject => {
  let value: unknown;

  try {
    value = z.toJSONSchema(schema, {
      io: 'output',
      unrepresentable: 'throw',
    });
  } catch (cause) {
    throw new ToolErrorObject(
      {
        code: 'invalid_schema',
        toolName: name,
        message: `Tool schema cannot be represented as JSON Schema: ${name}`,
      },
      { cause },
    );
  }

  const json = asJsonObject(value);

  if (json?.type !== 'object') {
    throw new ToolErrorObject({
      code: 'invalid_schema',
      toolName: name,
      message: `Tool schema must produce an object JSON Schema: ${name}`,
    });
  }

  return json;
};

const parseCall = (call: ToolCallRequest): ToolCall => {
  try {
    const payload = JSON.parse(call.arguments) as unknown;

    if (!isJsonValue(payload)) {
      throw new Error('Tool arguments must be JSON values.');
    }

    return {
      id: call.id,
      name: call.name,
      payload,
      index: call.index,
    };
  } catch (cause) {
    throw new ToolErrorObject(
      {
        code: 'invalid_json',
        toolName: call.name,
        callId: call.id,
        message: `Tool call arguments must be valid JSON: ${call.name}`,
        diagnostic: excerpt(call.arguments),
      },
      { cause },
    );
  }
};

const normalizeCall = (call: ToolCall | ToolCallRequest): ToolCall =>
  'payload' in call ? call : parseCall(call);

const validatePayload = <Schema extends ToolSchema>(
  tool: Tool<Schema>,
  call: ToolCall,
): z.output<Schema> => {
  const parsed = tool.schema.safeParse(call.payload);

  if (parsed.success) {
    return parsed.data;
  }

  throw new ToolErrorObject({
    code: 'invalid_payload',
    toolName: call.name,
    callId: call.id,
    message: `Tool call payload failed validation: ${call.name}`,
    issues: parsed.error.issues.map(issueFromZod),
  });
};

const issueFromZod = (issue: z.core.$ZodIssue): ToolIssue => ({
  path: issue.path.map(String).join('.'),
  message: issue.message,
});
