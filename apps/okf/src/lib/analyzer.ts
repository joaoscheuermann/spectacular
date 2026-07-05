import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import path from 'node:path';

import { collectStructure } from './collect.js';
import { extractFileFacts } from './extract.js';
import {
  appendHash,
  parseLog,
  renderLog,
  timestampForHash,
  type FolderLog,
} from './log.js';
import {
  renderDeterministicBody,
  renderFileConcept,
  renderIndex,
} from './markdown.js';
import {
  KNOWLEDGE_DIR,
  outputAbsolutePath,
  outputFolderPath,
  outputRelativePath,
  parentRelativePath,
} from './paths.js';
import { hashContent, readTextFile } from './read.js';
import { relationshipCandidatePaths } from './relationships.js';
import { buildTree, walk } from './tree.js';
import type {
  AnalyzedFile,
  CollectedEntry,
  DirectFileSummary,
  FileSummary,
  GenerateKnowledgeOptions,
  GenerateKnowledgeResult,
  ResolvedRelationship,
  SkippedFile,
} from './types.js';

type WrittenFile = {
  readonly directSummary: DirectFileSummary;
  readonly file: AnalyzedFile;
  readonly outputRelativePath: string;
  readonly relationships: readonly ResolvedRelationship[];
  readonly summary: FileSummary;
  readonly timestamp: string;
};

/** Generates an OKF knowledge bundle for a repository into .doric/knowledge. */
export const generateKnowledgeBundle = async (
  options: GenerateKnowledgeOptions,
): Promise<GenerateKnowledgeResult> => {
  const rootPath = path.resolve(options.rootPath);
  await assertDirectory(rootPath);
  options.logger?.info({ rootPath }, 'okf.start');

  const timestamp = (options.clock ?? (() => new Date()))().toISOString();
  const entries = await collectStructure(rootPath);
  options.logger?.info({ entries: entries.length }, 'okf.collect');
  const { files, skippedFiles } = await analyzeFiles(entries);
  options.logger?.info(
    { files: files.length, skippedFiles: skippedFiles.length },
    'okf.files',
  );
  const directoryPaths = directories(entries, files);
  const tree = buildTree(directoryPaths, files);
  const knowledgePath = path.join(rootPath, KNOWLEDGE_DIR);
  const logs = await readLogs(rootPath, directoryPaths);
  const written = new Map<string, WrittenFile>();

  await mkdir(knowledgePath, { recursive: true });

  for (const file of files) {
    const folderLog = logs.get(file.folderRelativePath) ?? new Map();
    const fileTimestamp = timestampForHash(
      folderLog,
      file.basename,
      file.hash,
      timestamp,
    );
    const summary = await options.summarizer.summarizeFile({
      file,
      deterministicBody: renderDeterministicBody(file),
      timestamp: fileTimestamp,
    });
    const outputPath = outputRelativePath(file.folderRelativePath, file.hash);
    const summaryEntry = buildDirectSummary(file, outputPath, summary);

    await writeFileConcept(rootPath, file, summary, fileTimestamp, []);
    written.set(file.relativePath, {
      directSummary: summaryEntry,
      file,
      outputRelativePath: outputPath,
      relationships: [],
      summary,
      timestamp: fileTimestamp,
    });
    logs.set(
      file.folderRelativePath,
      appendHash(folderLog, file.basename, file.hash, fileTimestamp),
    );
  }
  options.logger?.info({ files: written.size }, 'okf.summarize.files');

  let linkedRelationships = 0;
  for (const item of written.values()) {
    const relationships = resolveRelationships(item.file, written);

    if (relationships.length === 0) {
      continue;
    }

    linkedRelationships += relationships.length;
    await writeFileConcept(
      rootPath,
      item.file,
      item.summary,
      item.timestamp,
      relationships,
    );
    written.set(item.file.relativePath, { ...item, relationships });
  }
  options.logger?.info(
    { relationships: linkedRelationships },
    'okf.link.relationships',
  );

  let indexesWritten = 0;
  let logsWritten = 0;

  await walk(tree, async (directory) => {
    const folderPath = outputFolderPath(rootPath, directory.relativePath);
    await mkdir(folderPath, { recursive: true });

    const directFiles = directory.files
      .map((file) => written.get(file.relativePath)?.directSummary)
      .filter((summary): summary is DirectFileSummary => summary !== undefined);
    const childDirectories = directory.directories.map((child) =>
      path.posix.basename(child.relativePath),
    );
    const folderSummary = await options.summarizer.summarizeFolder({
      relativePath: directory.relativePath,
      directFiles,
      directories: childDirectories,
      timestamp,
    });

    await writeFile(
      path.join(folderPath, 'index.md'),
      renderIndex({
        relativePath: directory.relativePath,
        directFiles,
        directories: childDirectories,
        summary: folderSummary,
      }),
      'utf8',
    );
    indexesWritten += 1;

    await writeFile(
      path.join(folderPath, 'log.md'),
      renderLog(
        logs.get(directory.relativePath) ?? new Map(),
        directory.files.map((file) => file.basename),
      ),
      'utf8',
    );
    logsWritten += 1;
  });
  options.logger?.info(
    {
      knowledgePath,
      files: written.size,
      indexes: indexesWritten,
      logs: logsWritten,
    },
    'okf.complete',
  );

  return {
    rootPath,
    knowledgePath,
    filesWritten: written.size,
    indexesWritten,
    logsWritten,
    skippedFiles,
  };
};

const assertDirectory = async (rootPath: string): Promise<void> => {
  const stats = await stat(rootPath);

  if (!stats.isDirectory()) {
    throw new Error(`OKF --path must point to a directory: ${rootPath}`);
  }
};

const analyzeFiles = async (
  entries: readonly CollectedEntry[],
): Promise<{
  readonly files: readonly AnalyzedFile[];
  readonly skippedFiles: readonly SkippedFile[];
}> => {
  const files: AnalyzedFile[] = [];
  const skippedFiles: SkippedFile[] = [];

  for (const entry of entries.filter((item) => item.type === 'file')) {
    const read = await readTextFile(entry.absolutePath);

    if (read.type === 'skipped') {
      skippedFiles.push({
        relativePath: entry.relativePath,
        reason: read.reason,
      });
      continue;
    }

    const { extraction, kind } = extractFileFacts(
      entry.relativePath,
      read.content,
    );

    files.push({
      absolutePath: entry.absolutePath,
      basename: path.posix.basename(entry.relativePath),
      content: read.content,
      extension: path.posix.extname(entry.relativePath).toLowerCase(),
      extraction,
      folderRelativePath: parentRelativePath(entry.relativePath),
      hash: hashContent(read.buffer),
      kind,
      relativePath: entry.relativePath,
    });
  }

  return {
    files: files.sort((first, second) =>
      first.relativePath.localeCompare(second.relativePath),
    ),
    skippedFiles,
  };
};

const directories = (
  entries: readonly CollectedEntry[],
  files: readonly AnalyzedFile[],
): readonly string[] => {
  const values = new Set(
    entries
      .filter((entry) => entry.type === 'directory')
      .map((entry) => entry.relativePath),
  );

  for (const file of files) {
    addParents(values, file.folderRelativePath);
  }

  return [...values].filter((value) => value !== '').sort();
};

const addParents = (values: Set<string>, relativePath: string): void => {
  if (relativePath === '') {
    return;
  }

  values.add(relativePath);
  addParents(values, parentRelativePath(relativePath));
};

const readLogs = async (
  rootPath: string,
  directoryPaths: readonly string[],
): Promise<Map<string, FolderLog>> => {
  const logs = new Map<string, FolderLog>();

  for (const directoryPath of ['', ...directoryPaths]) {
    const logPath = path.join(
      outputFolderPath(rootPath, directoryPath),
      'log.md',
    );

    try {
      logs.set(directoryPath, parseLog(await readFile(logPath, 'utf8')));
    } catch {
      logs.set(directoryPath, new Map());
    }
  }

  return logs;
};

const writeFileConcept = async (
  rootPath: string,
  file: AnalyzedFile,
  summary: FileSummary,
  timestamp: string,
  relationships: readonly ResolvedRelationship[],
): Promise<void> => {
  const outputPath = outputRelativePath(file.folderRelativePath, file.hash);
  const absolutePath = outputAbsolutePath(rootPath, outputPath);

  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(
    absolutePath,
    renderFileConcept({ file, summary, timestamp, relationships }),
    'utf8',
  );
};

const buildDirectSummary = (
  file: AnalyzedFile,
  outputPath: string,
  summary: FileSummary,
): DirectFileSummary => ({
  title: file.extraction.title,
  description: summary.description?.trim() || file.extraction.description,
  sourceRelativePath: file.relativePath,
  outputFileName: path.posix.basename(outputPath),
  outputRelativePath: outputPath,
  summaryBody:
    summary.body?.trim() ||
    summary.description?.trim() ||
    file.extraction.description,
});

const resolveRelationships = (
  file: AnalyzedFile,
  written: ReadonlyMap<string, WrittenFile>,
): readonly ResolvedRelationship[] =>
  file.extraction.relationships
    .map((relationship) => {
      const target = resolveImport(file, relationship.specifier, written);

      if (target === undefined) {
        return undefined;
      }

      return {
        kind: relationship.kind,
        specifier: relationship.specifier,
        targetRelativePath: target.file.relativePath,
        targetOutputRelativePath: target.outputRelativePath,
      };
    })
    .filter(
      (relationship): relationship is ResolvedRelationship =>
        relationship !== undefined,
    );

const resolveImport = (
  file: AnalyzedFile,
  specifier: string,
  written: ReadonlyMap<string, WrittenFile>,
): WrittenFile | undefined => {
  const candidates = relationshipCandidatePaths(file, specifier);

  return candidates
    .map((candidate) => written.get(candidate))
    .find((value): value is WrittenFile => value !== undefined);
};
