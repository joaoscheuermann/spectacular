export type ConfigParseErrorCode =
  | 'invalid_message'
  | 'missing_first_part'
  | 'invalid_first_part_kind'
  | 'invalid_config_type'
  | 'invalid_config_field';

export type ConfigParseIssue = {
  readonly code: ConfigParseErrorCode;
  readonly path: string;
  readonly message: string;
};
