import crypto from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';

import { SYSTEM_PROMPT } from '../../prompts/analyze/index.js';
import { analyze, type Analysis } from './agents/analyze/index.js';
import { classify } from './agents/classify/index.js';
import type { Kind } from './agents/classify/kinds.js';
import { frontmatter, type Frontmatter } from './agents/frontmatter/index.js';
import { DEFAULT_BATCH_SIZE, DEFAULT_IGNORE } from './constants.js';
import { loadPrompts, type Prompts } from './prompts.js';
import { redactRegex } from './redact.js';
import {
  outputPath,
  parseDescription,
  renderTree,
  type TreeEntry,
} from './tree.js';
import type { GenerateResult, OkfConfig } from './types/okf.js';
import { walk } from './walk.js';

type Paths = {
  readonly root: string;
  readonly output: string;
};

type WorkContext = Paths & {
  readonly config: OkfConfig;
  readonly analyzePrompt: (kind: Kind) => Promise<string>;
  readonly prompts: Prompts;
};

type WorkResult = 'cached' | 'generated';

type Evidence = {
  readonly front: Frontmatter;
  readonly analysis: Analysis;
};

type ConceptInput = {
  readonly source: string;
  readonly hash: string;
  readonly front: Frontmatter;
  readonly summary: string;
};

type ResultInput = Paths & {
  readonly index: string;
  readonly files: readonly string[];
  readonly results: ReadonlyMap<string, WorkResult>;
};

/** Returns the conventional project-bundle output directory for a root. */
export const defaultOutput = (root: string): string =>
  path.join(path.resolve(root), '.agents', 'bundles', 'project');

/** Generates an OKF representation of a repository into its bundle root. */
export const generate = async (
  config: OkfConfig,
  root: string,
  output?: string,
): Promise<GenerateResult> => {
  const paths = await resolvePaths(root, output);
  const prompts = await loadPrompts(config.promptTarget);
  const analyzePrompt = SYSTEM_PROMPT(config.promptTarget ?? 'default');
  const context = { ...paths, config, prompts, analyzePrompt };
  const results = new Map<string, WorkResult>();

  await fs.mkdir(paths.output, { recursive: true });
  const files = await walk(
    paths.root,
    async (file, body) => {
      results.set(file, await work(context, file, body));
    },
    {
      batchSize: config.batchSize ?? DEFAULT_BATCH_SIZE,
      ignore: [...DEFAULT_IGNORE, ...(config.ignore ?? [])],
      signal: config.signal,
    },
  );
  const entries = await Promise.all(files.map((file) => entry(paths, file)));
  const index = path.join(paths.output, 'index.md');
  await fs.writeFile(index, renderTree(entries), 'utf-8');

  return result({ ...paths, index, files, results });
};

const resolvePaths = async (root: string, output?: string): Promise<Paths> => {
  const resolvedRoot = await fs.realpath(path.resolve(root));
  const stat = await fs.stat(resolvedRoot);
  if (!stat.isDirectory()) {
    throw new Error(`OKF root is not a directory: ${root}`);
  }

  const bundleRoot = path.join(resolvedRoot, '.agents', 'bundles');
  const resolvedOutput =
    output === undefined
      ? defaultOutput(resolvedRoot)
      : path.resolve(resolvedRoot, output);
  const relative = path.relative(bundleRoot, resolvedOutput);

  if (!isChild(relative)) {
    throw new Error(`OKF output must be inside ${bundleRoot}`);
  }

  const physicalBundle = await projectedRealPath(bundleRoot);
  const physicalOutput = await projectedRealPath(resolvedOutput);
  if (
    !isContained(resolvedRoot, physicalBundle) ||
    !isContained(physicalBundle, physicalOutput)
  ) {
    throw new Error(`OKF output must be inside ${bundleRoot}`);
  }

  return { root: resolvedRoot, output: resolvedOutput };
};

const projectedRealPath = async (target: string): Promise<string> => {
  let current = target;

  while (true) {
    try {
      const resolved = await fs.realpath(current);
      return path.resolve(resolved, path.relative(current, target));
    } catch (error) {
      if (!isAbsent(error)) throw error;
    }

    const parent = path.dirname(current);
    if (parent === current) throw new Error(`Cannot resolve path: ${target}`);
    current = parent;
  }
};

const isContained = (parent: string, child: string): boolean =>
  isChild(path.relative(parent, child));

const isChild = (relative: string): boolean =>
  relative.length > 0 &&
  relative !== '..' &&
  !relative.startsWith(`..${path.sep}`) &&
  !path.isAbsolute(relative);

const work = async (
  context: WorkContext,
  file: string,
  body: string,
): Promise<WorkResult> => {
  const relative = relativeFile(context.root, file);
  const target = path.join(context.output, outputPath(relative));
  const hash = crypto.createHash('sha256').update(body).digest('hex');
  if (await containsHash(target, hash)) return 'cached';

  const evidence = await describe(context, relative, redactRegex(body));
  await fs.mkdir(path.dirname(target), { recursive: true });
  await fs.writeFile(
    target,
    concept({
      source: relative,
      hash,
      front: evidence.front,
      summary: evidence.analysis.summary,
    }),
    'utf-8',
  );
  return 'generated';
};

const describe = async (
  context: WorkContext,
  source: string,
  content: string,
): Promise<Evidence> => {
  const completion = {
    provider: context.config.provider,
    model: context.config.model,
    effort: context.config.effort,
    signal: context.config.signal,
  };
  const classification = await classify(
    completion,
    context.prompts.classify,
    source,
    content,
  );
  const analyzePrompt = await context.analyzePrompt(classification.type);
  const front = await frontmatter(
    completion,
    context.prompts.frontmatter,
    source,
    content,
    classification,
  );
  const analysis = await analyze(
    completion,
    analyzePrompt,
    source,
    content,
    classification,
  );

  return { front, analysis };
};

const concept = (input: ConceptInput): string => `---
type: ${yamlString(input.front.type)}
title: ${yamlString(input.front.title)}
description: ${yamlString(input.front.description)}
resource: ${yamlString(`source:${input.source}`)}
tags: [${input.front.tags.map(yamlString).join(', ')}]
timestamp: ${yamlString(new Date().toISOString())}
hash: ${yamlString(input.hash)}
---

${input.summary}
`;

const entry = async (paths: Paths, file: string): Promise<TreeEntry> => {
  const relative = relativeFile(paths.root, file);
  const markdown = await fs.readFile(
    path.join(paths.output, outputPath(relative)),
    'utf-8',
  );

  return { path: relative, description: parseDescription(markdown) };
};

const result = (input: ResultInput): GenerateResult => {
  const values = [...input.results.values()];

  return {
    root: input.root,
    output: input.output,
    index: input.index,
    files: input.files.map((file) => relativeFile(input.root, file)),
    generated: values.filter((value) => value === 'generated').length,
    cached: values.filter((value) => value === 'cached').length,
  };
};

const relativeFile = (root: string, file: string): string => {
  const relative = path.relative(root, file);
  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    throw new Error(`Cannot represent a file outside the OKF root: ${file}`);
  }

  return relative.split(path.sep).join('/');
};

const containsHash = async (file: string, hash: string): Promise<boolean> => {
  try {
    return (await fs.readFile(file, 'utf-8')).includes(hash);
  } catch (error) {
    if (isMissing(error)) return false;
    throw error;
  }
};

const isMissing = (error: unknown): error is NodeJS.ErrnoException =>
  error instanceof Error && 'code' in error && error.code === 'ENOENT';

const isAbsent = (error: unknown): error is NodeJS.ErrnoException =>
  error instanceof Error &&
  'code' in error &&
  (error.code === 'ENOENT' || error.code === 'ENOTDIR');

const yamlString = (value: string): string => JSON.stringify(value);
