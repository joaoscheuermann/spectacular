export const BASE_TOOL_NAMES = [
  'list',
  'read',
  'write',
  'search',
  'calculate',
  'json_query',
] as const;

export const FINANCE_TOOL_NAMES = [
  'document_search',
  'document_fetch',
  'record_lookup',
  'timeseries_query',
  'currency_convert',
  'portfolio_snapshot',
] as const;

export const SOFTWARE_TOOL_NAMES = [
  'symbol_lookup',
  'dependency_query',
  'test_run',
  'build_check',
] as const;

export const ARTIFACT_TOOL_NAMES = [
  'template_get',
  'schema_validate',
  'render_preview',
  'artifact_publish',
] as const;

export const COMMUNICATION_TOOL_NAMES = [
  'channel_list',
  'recipient_resolve',
  'message_send',
  'message_status',
] as const;

export const TOOL_NAMES = [
  ...BASE_TOOL_NAMES,
  ...FINANCE_TOOL_NAMES,
  ...SOFTWARE_TOOL_NAMES,
  ...ARTIFACT_TOOL_NAMES,
  ...COMMUNICATION_TOOL_NAMES,
] as const;

export type ToolName = (typeof TOOL_NAMES)[number];

export const isToolName = (value: string): value is ToolName =>
  (TOOL_NAMES as readonly string[]).includes(value);
