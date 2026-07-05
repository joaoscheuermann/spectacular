export type FileKind = 'package-config' | 'configuration' | 'code' | 'text';

export type CollectedEntry = {
  readonly type: 'directory' | 'file';
  readonly absolutePath: string;
  readonly relativePath: string;
};

export type DescribedValue = {
  readonly name: string;
  readonly value: string;
  readonly description: string;
};

export type Declaration = {
  readonly kind: string;
  readonly name: string;
};

export type CodeExtraction = {
  readonly imports: readonly string[];
  readonly exports: readonly Declaration[];
  readonly declarations: readonly Declaration[];
  readonly methods: readonly string[];
};

export type PackageExtraction = {
  readonly scripts: readonly DescribedValue[];
  readonly dependencies: readonly DescribedValue[];
  readonly devDependencies: readonly DescribedValue[];
};

export type PendingRelationship = {
  readonly kind: 'imports';
  readonly specifier: string;
};

export type FileExtraction = {
  readonly type: string;
  readonly title: string;
  readonly description: string;
  readonly tags: readonly string[];
  readonly package?: PackageExtraction;
  readonly configKeys?: readonly string[];
  readonly code?: CodeExtraction;
  readonly relationships: readonly PendingRelationship[];
};

export type AnalyzedFile = {
  readonly absolutePath: string;
  readonly basename: string;
  readonly content: string;
  readonly extension: string;
  readonly extraction: FileExtraction;
  readonly folderRelativePath: string;
  readonly hash: string;
  readonly kind: FileKind;
  readonly relativePath: string;
};

export type DirectoryNode = {
  readonly relativePath: string;
  readonly files: readonly AnalyzedFile[];
  readonly directories: readonly DirectoryNode[];
};

export type FileSummaryInput = {
  readonly file: AnalyzedFile;
  readonly deterministicBody: string;
  readonly timestamp: string;
};

export type FileSummary = {
  readonly body?: string;
  readonly description?: string;
  readonly tags?: readonly string[];
};

export type DirectFileSummary = {
  readonly description: string;
  readonly outputFileName: string;
  readonly outputRelativePath: string;
  readonly sourceRelativePath: string;
  readonly summaryBody: string;
  readonly title: string;
};

export type FolderSummaryInput = {
  readonly directories: readonly string[];
  readonly directFiles: readonly DirectFileSummary[];
  readonly relativePath: string;
  readonly timestamp: string;
};

export type FolderSummary = {
  readonly body?: string;
  readonly description?: string;
};

export type KnowledgeSummarizer = {
  readonly summarizeFile: (input: FileSummaryInput) => Promise<FileSummary>;
  readonly summarizeFolder: (
    input: FolderSummaryInput,
  ) => Promise<FolderSummary>;
};

export type KnowledgeLogger = {
  readonly info: (
    details: Readonly<Record<string, unknown>>,
    message: string,
  ) => void;
  readonly warn?: (
    details: Readonly<Record<string, unknown>>,
    message: string,
  ) => void;
};

export type ResolvedRelationship = {
  readonly kind: 'imports';
  readonly specifier: string;
  readonly targetOutputRelativePath: string;
  readonly targetRelativePath: string;
};

export type GenerateKnowledgeOptions = {
  readonly clock?: () => Date;
  readonly logger?: KnowledgeLogger;
  readonly rootPath: string;
  readonly summarizer: KnowledgeSummarizer;
};

export type SkippedFile = {
  readonly reason: 'binary' | 'unreadable';
  readonly relativePath: string;
};

export type GenerateKnowledgeResult = {
  readonly filesWritten: number;
  readonly indexesWritten: number;
  readonly knowledgePath: string;
  readonly logsWritten: number;
  readonly rootPath: string;
  readonly skippedFiles: readonly SkippedFile[];
};
