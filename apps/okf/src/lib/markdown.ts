import type {
  AnalyzedFile,
  DirectFileSummary,
  FileSummary,
  FolderSummary,
  ResolvedRelationship,
} from './types.js';

export type RenderFileInput = {
  readonly file: AnalyzedFile;
  readonly relationships: readonly ResolvedRelationship[];
  readonly summary: FileSummary;
  readonly timestamp: string;
};

export type RenderIndexInput = {
  readonly directories: readonly string[];
  readonly directFiles: readonly DirectFileSummary[];
  readonly relativePath: string;
  readonly summary: FolderSummary;
};

export const renderDeterministicBody = (file: AnalyzedFile): string =>
  [
    sourceSection(file),
    packageSection(file),
    configSection(file),
    codeSection(file),
  ]
    .filter((section) => section !== '')
    .join('\n\n');

export const renderFileConcept = (input: RenderFileInput): string => {
  const description = frontmatterDescription(
    input.summary,
    input.file.extraction.description,
  );
  const tags = unique([
    ...input.file.extraction.tags,
    ...(input.summary.tags ?? []),
  ]);
  const body = [
    normalizedBody(input.summary.body, description),
    renderDeterministicBody(input.file),
    relationshipsSection(input.relationships),
  ]
    .filter((section) => section !== '')
    .join('\n\n');

  return [
    '---',
    `type: ${yamlString(input.file.extraction.type)}`,
    `title: ${yamlString(input.file.extraction.title)}`,
    `description: ${yamlString(description)}`,
    `resource: ${yamlString(`source:${input.file.relativePath}`)}`,
    `tags: [${tags.map(yamlString).join(', ')}]`,
    `timestamp: ${yamlString(input.timestamp)}`,
    '---',
    '',
    body,
    '',
  ].join('\n');
};

export const renderIndex = (input: RenderIndexInput): string => {
  const title =
    input.relativePath === '' ? 'Repository Knowledge' : input.relativePath;
  const body = [
    `# ${title}`,
    '',
    normalizedBody(
      input.summary.body,
      input.summary.description ??
        `Knowledge index for ${input.relativePath || 'the repository root'}.`,
    ),
    '',
    filesSection(input.directFiles),
    foldersSection(input.directories),
  ]
    .filter((section) => section !== '')
    .join('\n\n');

  return `${body.trimEnd()}\n`;
};

const sourceSection = (file: AnalyzedFile): string =>
  [
    '# Source',
    '',
    `- Path: \`${file.relativePath}\``,
    `- Content hash: \`${file.hash}\``,
    `- Kind: ${file.kind}`,
  ].join('\n');

const packageSection = (file: AnalyzedFile): string => {
  const manifest = file.extraction.package;

  if (manifest === undefined) {
    return '';
  }

  return [
    '# Package Scripts',
    '',
    table(['Script', 'Command', 'Description'], manifest.scripts),
    '',
    '# Dependencies',
    '',
    table(['Package', 'Version', 'Description'], manifest.dependencies),
    '',
    '# Dev Dependencies',
    '',
    table(['Package', 'Version', 'Description'], manifest.devDependencies),
  ].join('\n');
};

const configSection = (file: AnalyzedFile): string => {
  const keys = file.extraction.configKeys ?? [];

  if (keys.length === 0 || file.extraction.package !== undefined) {
    return '';
  }

  return [
    '# Configuration Keys',
    '',
    ...keys.map((key) => `- \`${key}\``),
  ].join('\n');
};

const codeSection = (file: AnalyzedFile): string => {
  const code = file.extraction.code;

  if (code === undefined) {
    return '';
  }

  return [
    '# Exports',
    '',
    declarationList(code.exports),
    '',
    '# Declarations',
    '',
    declarationList(code.declarations),
    '',
    '# Methods',
    '',
    code.methods.length === 0
      ? '- No method-like declarations detected.'
      : code.methods.map((method) => `- \`${method}\``).join('\n'),
    '',
    '# Imports',
    '',
    code.imports.length === 0
      ? '- No imports detected.'
      : code.imports.map((specifier) => `- \`${specifier}\``).join('\n'),
  ].join('\n');
};

const relationshipsSection = (
  relationships: readonly ResolvedRelationship[],
): string => {
  if (relationships.length === 0) {
    return '';
  }

  return [
    '# Relationships',
    '',
    ...relationships.map(
      (relationship) =>
        `- Imports [${relationship.targetRelativePath}](/${relationship.targetOutputRelativePath}) via \`${relationship.specifier}\`.`,
    ),
  ].join('\n');
};

const filesSection = (files: readonly DirectFileSummary[]): string => {
  if (files.length === 0) {
    return '## Files\n\nNo direct file summaries.';
  }

  return [
    '## Files',
    '',
    ...files.map(
      (file) =>
        `* [${file.title}](${file.outputFileName}) - ${file.description}`,
    ),
  ].join('\n');
};

const foldersSection = (directories: readonly string[]): string => {
  if (directories.length === 0) {
    return '';
  }

  return [
    '## Folders',
    '',
    ...directories.map(
      (directory) =>
        `* [${directory}](${directory}/) - Knowledge for source folder \`${directory}\`.`,
    ),
  ].join('\n');
};

const table = (
  headers: readonly string[],
  rows: readonly {
    readonly description: string;
    readonly name: string;
    readonly value: string;
  }[],
): string => {
  if (rows.length === 0) {
    return 'No entries detected.';
  }

  return [
    `| ${headers.join(' |')} |`,
    `| ${headers.map(() => '---').join(' | ')} |`,
    ...rows.map(
      (row) =>
        `| \`${escapeCell(row.name)}\` | \`${escapeCell(row.value)}\` | ${escapeCell(row.description)} |`,
    ),
  ].join('\n');
};

const declarationList = (
  declarations: readonly { readonly kind: string; readonly name: string }[],
): string =>
  declarations.length === 0
    ? '- No declarations detected.'
    : declarations
        .map((declaration) => `- \`${declaration.name}\` (${declaration.kind})`)
        .join('\n');

const normalizedBody = (
  body: string | undefined,
  fallbackDescription: string,
): string => {
  const value = body?.trim();

  if (value !== undefined && value !== '') {
    return value;
  }

  return ['# Summary', '', fallbackDescription].join('\n');
};

const yamlString = (value: string): string => JSON.stringify(value);

const escapeCell = (value: string): string => value.replaceAll('|', '\\|');

const unique = (values: readonly string[]): readonly string[] => [
  ...new Set(values.filter((value) => value.trim() !== '')),
];

const frontmatterDescription = (
  summary: FileSummary,
  fallbackDescription: string,
): string => {
  const direct = summary.description?.trim();

  if (direct !== undefined && direct !== '') {
    return oneLine(direct);
  }

  const fromBody = firstSentence(summary.body ?? '');

  return fromBody === '' ? oneLine(fallbackDescription) : fromBody;
};

const firstSentence = (markdown: string): string => {
  const plain = markdown
    .replace(/```[\s\S]*?```/gu, ' ')
    .replace(/^#{1,6}\s+.+$/gmu, ' ')
    .replace(/[*_`>#-]/gu, ' ')
    .replace(/\s+/gu, ' ')
    .trim();
  const sentence = /(.+?[.!?])(?:\s|$)/u.exec(plain)?.[1] ?? plain;

  return oneLine(sentence);
};

const oneLine = (value: string): string => {
  const text = value.replace(/\s+/gu, ' ').trim();

  if (text.length <= 180) {
    return text;
  }

  return `${text.slice(0, 177).trimEnd()}...`;
};
