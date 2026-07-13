export const KINDS = [
  'source_code',
  'test',
  'configuration',
  'package_manifest',
  'lockfile',
  'build',
  'documentation',
  'generated',
  'schema_contract',
  'database',
  'script',
  'ci_cd',
  'infrastructure',
  'deployment_runtime',
  'static_asset',
  'template',
  'localization',
  'data_fixture',
  'editor_tooling',
  'version_control_metadata',
  'legal_governance',
  'secret_related',
  'binary_vendor',
  'none',
] as const;

export type Kind = (typeof KINDS)[number];

export const isKind = (value: string): value is Kind =>
  (KINDS as readonly string[]).includes(value);
