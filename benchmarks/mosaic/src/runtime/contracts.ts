import { z } from 'zod';

import type { ToolName } from '../config/index.js';

const text = z.string().trim().min(1);
const finite = z.number().finite();
const json = z.json();
const empty = z.object({}).strict();
const targetResult = z.object({ target: text, result: json }).strict();

export interface ToolContract {
  readonly description: string;
  readonly input: z.ZodObject;
  readonly output: z.ZodType;
}

/** Exact model-facing schemas for the 24 deterministic world operations. */
export const TOOL_CONTRACTS: Readonly<Record<ToolName, ToolContract>> = {
  list: {
    description:
      'List fixture file paths, optionally restricted to a path prefix.',
    input: z.object({ prefix: z.string().optional() }).strict(),
    output: z.array(z.string()),
  },
  read: {
    description: 'Read the exact UTF-8 content of one fixture file path.',
    input: z.object({ path: text }).strict(),
    output: z.object({ path: text, content: z.string() }).strict(),
  },
  write: {
    description:
      'Write UTF-8 content to one path inside the isolated run world.',
    input: z.object({ path: text, content: text }).strict(),
    output: z
      .object({ path: text, bytes: z.number().int().nonnegative() })
      .strict(),
  },
  search: {
    description:
      'Search fixture file contents for a case-insensitive text query.',
    input: z.object({ query: text }).strict(),
    output: z.object({ query: text, matches: z.array(z.string()) }).strict(),
  },
  calculate: {
    description:
      'Apply one deterministic arithmetic operation to a non-empty list of finite numbers.',
    input: z
      .object({
        operation: z.enum(['add', 'subtract', 'multiply', 'divide', 'sum']),
        values: z.array(finite).min(1),
      })
      .strict(),
    output: z.object({ operation: text, result: finite }).strict(),
  },
  json_query: {
    description:
      'Read a JSON fixture file and return the value at a dot-separated property path.',
    input: z.object({ path: text, query: text }).strict(),
    output: json,
  },
  document_search: {
    description:
      'Search finance-document titles and bodies for a case-insensitive query.',
    input: z.object({ query: text }).strict(),
    output: z.array(z.object({ id: text, title: text }).strict()),
  },
  document_fetch: {
    description: 'Fetch one finance document by its exact fixture identifier.',
    input: z.object({ id: text }).strict(),
    output: z.object({ id: text, title: text, text: z.string() }).strict(),
  },
  record_lookup: {
    description:
      'Look up one authoritative ledger record by exact fixture identifier.',
    input: z.object({ id: text }).strict(),
    output: z.object({ id: text, record: json }).strict(),
  },
  timeseries_query: {
    description:
      'Return all ordered observations for one named fixture time series.',
    input: z.object({ series: text }).strict(),
    output: z.object({ series: text, values: z.array(finite) }).strict(),
  },
  currency_convert: {
    description:
      'Convert a finite amount with the exact fixture exchange rate for two currency codes.',
    input: z.object({ from: text, to: text, amount: finite }).strict(),
    output: z
      .object({
        from: text,
        to: text,
        amount: finite,
        rate: finite,
        converted: finite,
      })
      .strict(),
  },
  portfolio_snapshot: {
    description: 'Return the complete isolated portfolio-position fixture.',
    input: empty,
    output: z.array(
      z
        .object({ id: text, assetClass: text, currency: text, value: finite })
        .strict(),
    ),
  },
  symbol_lookup: {
    description: 'Look up fixture metadata for one exact software symbol name.',
    input: z.object({ name: text }).strict(),
    output: z.object({ name: text, symbol: json }).strict(),
  },
  dependency_query: {
    description:
      'Query direct dependencies or direct dependents for one fixture component.',
    input: z
      .object({
        component: text,
        direction: z.enum(['dependencies', 'dependents']).optional(),
      })
      .strict(),
    output: z
      .object({ component: text, direction: text, values: z.array(z.string()) })
      .strict(),
  },
  test_run: {
    description:
      'Run one deterministic fixture test target and return its stored result.',
    input: z.object({ target: text }).strict(),
    output: targetResult,
  },
  build_check: {
    description:
      'Run a deterministic fixture build check for a named target or the full build.',
    input: z.object({ target: text.optional() }).strict(),
    output: targetResult,
  },
  template_get: {
    description: 'Fetch one artifact template by its exact fixture name.',
    input: z.object({ name: text }).strict(),
    output: z.object({ name: text, template: json }).strict(),
  },
  schema_validate: {
    description:
      'Validate a JSON value against one named fixture artifact schema.',
    input: z.object({ name: text, value: json }).strict(),
    output: z
      .object({ valid: z.boolean(), missing: z.array(z.string()) })
      .strict(),
  },
  render_preview: {
    description:
      'Render a deterministic canonical preview of a JSON artifact value.',
    input: z
      .object({ format: text.optional(), value: json.optional() })
      .strict(),
    output: z.object({ format: text, rendered: z.string() }).strict(),
  },
  artifact_publish: {
    description:
      'Publish one JSON value into the isolated run artifact registry.',
    input: z.object({ value: json.optional() }).strict(),
    output: z.object({ id: text, published: z.literal(true) }).strict(),
  },
  channel_list: {
    description: 'List all communication channels in the isolated fixture.',
    input: empty,
    output: z.array(z.object({ name: text, id: text }).strict()),
  },
  recipient_resolve: {
    description:
      'Resolve a recipient query to zero or more exact fixture destination identifiers.',
    input: z.object({ query: text }).strict(),
    output: z.object({ query: text, candidates: z.array(z.string()) }).strict(),
  },
  message_send: {
    description:
      'Send one message to an exact resolved recipient inside the isolated world.',
    input: z
      .object({ recipientId: text, channelId: text.optional(), body: text })
      .strict(),
    output: z.object({ id: text, status: z.literal('delivered') }).strict(),
  },
  message_status: {
    description:
      'Read the observed delivery status of one exact fixture message identifier.',
    input: z.object({ id: text }).strict(),
    output: z
      .object({ id: text, recipientId: text, status: z.literal('delivered') })
      .strict(),
  },
};
