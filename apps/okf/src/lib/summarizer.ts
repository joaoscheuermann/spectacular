import { createAgent } from 'agent';
import { createFetchTransport, createLmStudioProvider } from 'llms';
import { createMessageStorage } from 'messages';
import { createToolStorage } from 'tools';

import type {
  DirectFileSummary,
  FileSummary,
  FileSummaryInput,
  FolderSummary,
  FolderSummaryInput,
  KnowledgeSummarizer,
} from './types.js';

export const LM_STUDIO_BASE_URL = 'http://127.0.0.1:1234';
export const OKF_MODEL = 'google/gemma-4-e4b';

export const SUMMARIZER_STYLE_RULES = [
  'Write clean, direct, neutral language.',
  'Do not use emoji or decorative symbols.',
  'Do not use hype, marketing language, or inflated claims.',
  'Do not hallucinate. Do not invent behavior, intent, relationships, or context.',
  'Do not make assumptions. Use only information present in the supplied facts and content excerpt.',
  'If the available information is insufficient, state that directly.',
].join('\n');

export type LmStudioSummarizerOptions = {
  readonly baseUrl?: string;
  readonly model?: string;
};

/** Creates the production LM Studio-backed summarizer used by the CLI. */
export const createLmStudioSummarizer = (
  options: LmStudioSummarizerOptions = {},
): KnowledgeSummarizer => {
  const model = options.model ?? OKF_MODEL;
  const provider = createLmStudioProvider({
    transport: createFetchTransport(),
    baseUrl: options.baseUrl ?? LM_STUDIO_BASE_URL,
  });

  const complete = async (input: string): Promise<string> => {
    const agent = createAgent({
      provider,
      tools: createToolStorage([]),
      messages: createMessageStorage(),
      system: [
        'You generate concise Open Knowledge Format markdown bodies.',
        'Return markdown only, without YAML frontmatter.',
        SUMMARIZER_STYLE_RULES,
      ].join('\n'),
      model,
      temperature: 0,
      maxOutputTokens: 900,
    });

    return (await agent.complete(input)).text.trim();
  };

  return {
    summarizeFile: async (input) => {
      const body = await complete(filePrompt(input));

      return {
        body,
      };
    },
    summarizeFolder: async (input) => ({
      body: await complete(folderPrompt(input)),
      description: `Knowledge index for ${input.relativePath || 'the repository root'}.`,
    }),
  };
};

/** Deterministic summarizer for tests and non-network callers. */
export const createStaticSummarizer = (): KnowledgeSummarizer => ({
  summarizeFile: async (input): Promise<FileSummary> => ({
    description: input.file.extraction.description,
    body: ['# Summary', '', input.file.extraction.description].join('\n'),
  }),
  summarizeFolder: async (input): Promise<FolderSummary> => ({
    description: `Knowledge index for ${input.relativePath || 'the repository root'}.`,
    body: [
      '# Folder Summary',
      '',
      input.directFiles.length === 0
        ? 'No direct file summaries were generated for this folder.'
        : [
            `Direct summaries: ${input.directFiles
              .map((file) => file.sourceRelativePath)
              .join(', ')}.`,
            '',
            ...input.directFiles.map(staticDirectFileSummary),
          ].join('\n'),
    ].join('\n'),
  }),
});

const filePrompt = (input: FileSummaryInput): string =>
  [
    'Summarize this repository file as an OKF concept body.',
    'Keep the response short and preserve the deterministic facts below.',
    'Describe only what the file does based on the supplied facts and excerpt.',
    '',
    '# Style Rules',
    '',
    SUMMARIZER_STYLE_RULES,
    '',
    `Path: ${input.file.relativePath}`,
    `Kind: ${input.file.kind}`,
    `Timestamp: ${input.timestamp}`,
    '',
    input.deterministicBody,
    '',
    '# Content Excerpt',
    '',
    fenced(input.file.content.slice(0, 12000)),
  ].join('\n');

const folderPrompt = (input: FolderSummaryInput): string =>
  [
    'Summarize this repository folder for an OKF index.md file.',
    'Use all direct file summaries listed here.',
    'Describe only what the folder contains based on the supplied summaries.',
    '',
    '# Style Rules',
    '',
    SUMMARIZER_STYLE_RULES,
    '',
    `Folder: ${input.relativePath || '.'}`,
    `Timestamp: ${input.timestamp}`,
    '',
    '# Direct Files',
    '',
    input.directFiles.length === 0
      ? 'No direct files.'
      : input.directFiles.map(promptDirectFileSummary).join('\n\n'),
    '',
    '# Direct Folders',
    '',
    input.directories.length === 0
      ? 'No direct folders.'
      : input.directories.map((directory) => `- ${directory}`).join('\n'),
  ].join('\n');

const promptDirectFileSummary = (file: DirectFileSummary): string =>
  [
    `## ${file.sourceRelativePath}`,
    '',
    `Title: ${file.title}`,
    `Description: ${file.description}`,
    `Output file: ${file.outputFileName}`,
    `Output path: ${file.outputRelativePath}`,
    '',
    'Generated file summary:',
    '',
    file.summaryBody.trim() || 'No generated file summary body.',
  ].join('\n');

const staticDirectFileSummary = (file: DirectFileSummary): string =>
  [`## ${file.sourceRelativePath}`, '', file.summaryBody].join('\n');

const fenced = (content: string): string =>
  ['```text', content.replaceAll('```', '\\`\\`\\`'), '```'].join('\n');
