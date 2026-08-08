import {
  structuredJsonSchema,
  type ProviderFinished,
  type StructuredOutputSchema,
} from 'llms';
import type { ToolDefinition } from 'tool';

import { AgentErrorObject } from '../classes/agent-error.js';

const baseName = 'submit_structured_output';
const description = 'Submit the final structured output and end the agent run.';
const invalidSubmissionLimit = 4;
const validationIssueLimit = 10;
const terminalOutputSchema = { not: {} } as const;

export type StructuredOutputTool = {
  readonly name: string;
  readonly definition: ToolDefinition;
  readonly schema: StructuredOutputSchema;
};

export type StructuredOutputSubmission<Output> =
  | { readonly type: 'continue' }
  | { readonly type: 'finished'; readonly finish: ProviderFinished<Output> }
  | { readonly type: 'invalid'; readonly error: AgentErrorObject };

export type StructuredOutputRepair = {
  readonly invalidSubmissions: number;
  readonly correction: string;
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
    definition: {
      name,
      description,
      inputSchema,
      outputSchema: terminalOutputSchema,
      strict: true,
    },
  };
};

/** Instructs the model how to end a tool-enabled structured run. */
export const structuredOutputInstruction = (name: string): string =>
  [
    `When the task is complete, call \`${name}\` exactly once with the final structured output.`,
    'That call must be the only tool call in its response.',
    'Do not return the final output as ordinary text.',
  ].join(' ');

/** Classifies a terminal submission without executing or persisting invalid calls. */
export const parseStructuredOutputTool = <Output>(
  finish: ProviderFinished<unknown>,
  tool: StructuredOutputTool,
): StructuredOutputSubmission<Output> => {
  /** Ordinary tool calls are not terminal and continue through the agent loop. */
  const terminal = finish.toolCalls.filter(({ name }) => name === tool.name);

  if (terminal.length === 0) {
    return finish.toolCalls.length === 0
      ? invalidSubmission('Agent run ended without terminal structured output.')
      : { type: 'continue' };
  }

  /** A mixed response is ambiguous: no executable call may accompany completion. */
  if (terminal.length !== 1 || finish.toolCalls.length !== 1) {
    return invalidSubmission(
      'Agent terminal structured output must be the only tool call.',
    );
  }

  let value: unknown;

  /** Function arguments arrive as a JSON string at the provider boundary. */
  try {
    value = JSON.parse(terminal[0].arguments) as unknown;
  } catch {
    return invalidSubmission(
      'Agent terminal structured output arguments must be valid JSON.',
    );
  }

  /**
   * Validate locally with the original Zod schema. This enforces refinements
   * that may not be representable in the provider-facing JSON Schema.
   */
  const parsed = tool.schema.safeParse(value);

  if (!parsed.success) {
    return invalidSubmission(
      'Agent terminal structured output failed schema validation.',
      validationDiagnostic(parsed.error.issues),
    );
  }

  /**
   * Convert the internal terminal call into the same tool-free finish shape
   * returned by provider-native structured output.
   */
  return {
    type: 'finished',
    finish: {
      ...finish,
      text: terminal[0].arguments,
      finishReason: 'stop',
      toolCalls: [],
      structured: parsed.data as Output,
    },
  };
};

/** Applies the fixed cumulative budget and prepares one transient correction. */
export const nextStructuredOutputRepair = (
  tool: StructuredOutputTool,
  error: AgentErrorObject,
  invalidSubmissions: number,
): StructuredOutputRepair => {
  const nextInvalidSubmissions = invalidSubmissions + 1;

  if (nextInvalidSubmissions >= invalidSubmissionLimit) throw error;

  return {
    invalidSubmissions: nextInvalidSubmissions,
    correction: structuredOutputCorrection(tool.name, error),
  };
};

const availableName = (names: ReadonlySet<string>): string => {
  if (!names.has(baseName)) return baseName;

  /** Select the first deterministic suffix not owned by caller tools. */
  let suffix = 2;
  while (names.has(`${baseName}_${suffix}`)) suffix += 1;

  return `${baseName}_${suffix}`;
};

const structuredOutputCorrection = (
  name: string,
  error: AgentErrorObject,
): string =>
  [
    '# Structured output correction',
    '',
    `The previous response was rejected: ${error.data.message}`,
    '',
    `Submit the corrected final output by calling \`${name}\` exactly once. That call must be the only tool call in its response.`,
    ...(error.data.diagnostic === undefined
      ? []
      : ['', `Validation issues: ${error.data.diagnostic}`]),
  ].join('\n');

const validationDiagnostic = (
  issues: readonly {
    readonly path: readonly PropertyKey[];
    readonly message: string;
  }[],
): string =>
  issues
    .slice(0, validationIssueLimit)
    .map((issue) => {
      const path = issue.path.map(String).join('.');
      const message = issue.message.replace(/\s+/g, ' ').trim();

      return path === '' ? message : `${path}: ${message}`;
    })
    .join('; ');

const invalidSubmission = <Output>(
  message: string,
  diagnostic?: string,
): StructuredOutputSubmission<Output> => ({
  type: 'invalid',
  error: new AgentErrorObject({
    code: 'invalid_structured_output',
    message,
    ...(diagnostic === undefined || diagnostic.length === 0
      ? {}
      : { diagnostic }),
  }),
});
