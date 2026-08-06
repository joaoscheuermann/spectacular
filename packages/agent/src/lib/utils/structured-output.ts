import {
  structuredJsonSchema,
  type ProviderFinished,
  type StructuredOutputSchema,
} from 'llms';
import type { ToolDefinition } from 'tool';

import { AgentErrorObject } from '../classes/agent-error.js';

const baseName = 'submit_structured_output';
const description = 'Submit the final structured output and end the agent run.';

export type StructuredOutputTool = {
  readonly name: string;
  readonly definition: ToolDefinition;
  readonly schema: StructuredOutputSchema;
};

/**
 * Creates a collision-free strict tool for terminal structured output. MOSAIC
 * Algorithm 1 deliberately leaves the tool-calling protocol open; this helper
 * is the agent package's provider-neutral implementation, not a MOSAIC tool.
 */
export const createStructuredOutputTool = (
  provider: string,
  schema: StructuredOutputSchema,
  tools: readonly ToolDefinition[],
): StructuredOutputTool => {
  /** Never shadow a caller tool that already uses the preferred terminal name. */
  const name = availableName(new Set(tools.map((tool) => tool.name)));

  /** Reuse the provider schema conversion used by native structured output. */
  const inputSchema = structuredJsonSchema(provider, schema);

  if (inputSchema === undefined) {
    throw new AgentErrorObject({
      code: 'invalid_structured_output',
      message: 'Agent terminal structured output requires an object schema.',
    });
  }

  return {
    name,
    schema,
    definition: { name, description, inputSchema, strict: true },
  };
};

/** Instructs the model how to end a tool-enabled structured run. */
export const structuredOutputInstruction = (name: string): string =>
  [
    `When the task is complete, call \`${name}\` exactly once with the final structured output.`,
    'That call must be the only tool call in its response.',
    'Do not return the final output as ordinary text.',
  ].join(' ');

/** Converts the reserved terminal tool call into a validated provider finish. */
export const parseStructuredOutputTool = <Output>(
  finish: ProviderFinished<unknown>,
  tool: StructuredOutputTool,
): ProviderFinished<Output> | undefined => {
  /** Ordinary tool calls are not terminal and continue through the agent loop. */
  const terminal = finish.toolCalls.filter(({ name }) => name === tool.name);

  if (terminal.length === 0) return undefined;

  /** A mixed response is ambiguous: no executable call may accompany completion. */
  if (terminal.length !== 1 || finish.toolCalls.length !== 1) {
    throw invalidOutput(
      'Agent terminal structured output must be the only tool call.',
    );
  }

  let value: unknown;

  /** Function arguments arrive as a JSON string at the provider boundary. */
  try {
    value = JSON.parse(terminal[0].arguments) as unknown;
  } catch {
    throw invalidOutput(
      'Agent terminal structured output arguments must be valid JSON.',
    );
  }

  /**
   * Validate locally with the original Zod schema. This enforces refinements
   * that may not be representable in the provider-facing JSON Schema.
   */
  const parsed = tool.schema.safeParse(value);

  if (!parsed.success) {
    throw invalidOutput(
      'Agent terminal structured output failed schema validation.',
    );
  }

  /**
   * Convert the internal terminal call into the same tool-free finish shape
   * returned by provider-native structured output.
   */
  return {
    ...finish,
    text: terminal[0].arguments,
    finishReason: 'stop',
    toolCalls: [],
    structured: parsed.data as Output,
  };
};

export const missingStructuredOutput = (): AgentErrorObject =>
  invalidOutput('Agent run ended without terminal structured output.');

const availableName = (names: ReadonlySet<string>): string => {
  if (!names.has(baseName)) return baseName;

  /** Select the first deterministic suffix not owned by caller tools. */
  let suffix = 2;
  while (names.has(`${baseName}_${suffix}`)) suffix += 1;

  return `${baseName}_${suffix}`;
};

const invalidOutput = (message: string): AgentErrorObject =>
  new AgentErrorObject({ code: 'invalid_structured_output', message });
