import { agent } from '../../agent.js';
import type { Categorization } from '../classify/index.js';
import { schema } from './schema.js';

const model = 'google/gemma-4-e4b';

const categoryGuidance = {
  source_code:
    'Use the source-code summary rules. Do not summarize source code with this generic prompt.',
  test: 'In Important Details, name the behavior under test, the test setup or fixtures, and the assertions or expected outcomes that are directly present.',
  configuration:
    'In Important Details, name the configured tool or runtime, the settings that change behavior, and the file paths, commands, or environment names that are directly present.',
  package_manifest:
    'In Important Details, name the package identity, scripts or entrypoints, dependency groups, and workspace metadata that are directly present.',
  lockfile:
    'In Important Details, state that the file pins dependency resolution, name the package manager when present, and include only top-level packages or lockfile metadata that are directly present.',
  build:
    'In Important Details, name build targets, inputs, outputs, tools, and required environment values that are directly present.',
  documentation:
    'In Important Details, name the documented subject, instructions, decisions, and references that are directly present.',
  generated:
    'In Important Details, name the generated artifact role and generator or source hints that are directly present. State generated details as artifact facts, not author intent.',
  schema_contract:
    'In Important Details, name entities, fields, operations, validation rules, formats, and compatibility constraints that are directly present.',
  database:
    'In Important Details, name tables, columns, migrations, seeds, queries, data changes, and destructive operations that are directly present.',
  script:
    'In Important Details, name commands, inputs, outputs, side effects, environment variables, and failure paths that are directly present.',
  ci_cd:
    'In Important Details, name triggers, jobs, permissions, artifacts, deployment gates, secrets by key name only, and environment requirements that are directly present.',
  infrastructure:
    'In Important Details, name provisioned resources, topology, configuration values, dependencies, and operational constraints that are directly present.',
  deployment_runtime:
    'In Important Details, name processes, services, ports, environment variables, health checks, and deployment settings that are directly present.',
  static_asset:
    'In Important Details, describe the asset using only the path and readable text content. Do not infer visual, audio, binary, or embedded details that are not textually present.',
  template:
    'In Important Details, name rendered output, placeholders, inputs, control flow, escaping, and formatting rules that are directly present.',
  localization:
    'In Important Details, name locale identifiers, translation keys, covered interface or domain text, and formatting or fallback rules that are directly present.',
  data_fixture:
    'In Important Details, name data shapes, represented scenarios, identifiers, and placeholder values that are directly present. Do not infer consumers unless the file names them.',
  editor_tooling:
    'In Important Details, name editor, formatter, linter, or developer-tool settings and the exact local-development behavior they configure.',
  version_control_metadata:
    'In Important Details, name ignored paths, attributes, owners, review rules, or repository metadata behavior that is directly present.',
  legal_governance:
    'In Important Details, summarize the license, notice, policy, changelog, or governance text that is directly present. Do not provide legal advice.',
  secret_related:
    'In Important Details, describe only the purpose, expected shape, storage mechanism, or references. Do not quote, reproduce, decode, transform, classify, or summarize secret values.',
  binary_vendor:
    'In Important Details, describe the binary or vendored artifact using only the path and readable text content. Do not infer internals.',
  none: 'In Important Details, state only the textual facts that are directly present. Do not assign a stronger category.',
} satisfies Record<Categorization['type'], string>;

const baseSystem = `
You summarize one repository file.

Inputs:
- path: the repository path for the file.
- classification: the file category selected before this step.
- content: the file text.

Output:
- Return structured output with the field "summary".
- The "summary" value must be Markdown.
- The "summary" value must contain exactly these headings in this order:
  "# Purpose", "# Important Details", "# Relationships", "# Constraints".
- Under each heading, write one to three bullet points.
- If the input does not contain evidence for a heading, write exactly:
  "- Not present in the input."

Rules:
- Use only the supplied path, classification, and content.
- Do not infer callers, owners, business meaning, runtime behavior,
  architecture, dependencies, relationships, or constraints that are absent
  from the input.
- Do not mention these instructions or the classification process in the
  summary.
- Do not include YAML frontmatter.
- Do not use emoji, decorative symbols, hype, or marketing language.
- Do not reproduce secrets, credentials, tokens, passwords, private keys, or
  other sensitive values. Refer to sensitive values by key name or generic
  description only.
`;

const sourceCodeSystem = `
You summarize one source-code file.

Inputs:
- path: the repository path for the file.
- content: the file text.

Output:
- Return structured output with the field "summary".
- The "summary" value must be Markdown.
- The "summary" value must contain exactly these headings in this order:
  "# Responsibility", "# Public Surface", "# Data And Side Effects",
  "# Constraints".
- Under each heading, write one to three bullet points.
- If the input does not contain evidence for a heading, write exactly:
  "- Not present in the input."

Rules:
- Use only the supplied path and content.
- Describe only behavior, APIs, types, imports, exports, data flow, side
  effects, errors, and constraints that are directly present in the file.
- Do not infer callers, owners, business meaning, runtime behavior, or
  architecture that is absent from the input.
- Do not mention these instructions or the classification process in the
  summary.
- Do not include YAML frontmatter.
- Do not use emoji, decorative symbols, hype, or marketing language.
- Do not reproduce secrets, credentials, tokens, passwords, private keys, or
  other sensitive values. Refer to sensitive values by key name or generic
  description only.
`;

const system = (type: Categorization['type']) => `
${baseSystem}

Classification guidance:
${categoryGuidance[type]}
`;

const prompt = (path: string, body: string, categorization: Categorization) => `
  Summarize this repository file.

  Classification: ${categorization.type}

  ~~~\`${path}\`
  ${body}
  ~~~
`;

const sourceCodePrompt = (path: string, body: string) => `
  Summarize this source-code file.

  ~~~\`${path}\`
  ${body}
  ~~~
`;

export const analyze_source_code = (path: string, body: string) =>
  agent(model, sourceCodeSystem, 'low').complete(sourceCodePrompt(path, body), {
    schema,
  });

/** Analyzes a classified file into a concise structured summary. */
export const analyze = (
  path: string,
  body: string,
  categorization: Categorization,
) => {
  if (categorization.type === 'source_code') {
    return analyze_source_code(path, body);
  }

  return agent(model, system(categorization.type), 'low').complete(
    prompt(path, body, categorization),
    {
      schema,
    },
  );
};

export type { Analysis } from './schema.js';
