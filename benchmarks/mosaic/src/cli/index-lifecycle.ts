import { access, mkdir, open, readFile, readdir } from 'node:fs/promises';
import { basename, join } from 'node:path';

import type { LlmProvider } from 'llms';
import { z } from 'zod';

import { mosaicSkills } from '../conditions/index.js';
import { EMBEDDING_MODEL } from '../config/index.js';
import { artifactHash, canonicalJson } from '../core/index.js';
import type { MeteredProvider, MeterUsage } from './provider.js';
import type { MeteringStore } from './metering-store.js';

export const RETRIEVAL_ALGORITHM =
  'victor-bm25+cosine+rrf-k60/content-index-v1';

const hash = z.string().regex(/^sha256:[a-f0-9]{64}$/u);
const entry = z
  .object({
    view: z.enum(['body', 'metadata']),
    skillName: z.string().trim().min(1),
    text: z.string(),
    textBytes: z.number().int().safe().nonnegative(),
    textHash: hash,
    vector: z.array(z.number().finite()).min(1),
  })
  .strict();
const recipe = z
  .object({
    schemaVersion: z.literal(1),
    catalogHash: hash,
    embedder: z.string().trim().min(1),
    dimensions: z.number().int().safe().positive(),
    algorithm: z.literal(RETRIEVAL_ALGORITHM),
    views: z.array(
      z
        .object({
          view: z.enum(['body', 'metadata']),
          entries: z.array(
            z
              .object({
                skillName: z.string().trim().min(1),
                text: z.string(),
                textBytes: z.number().int().safe().nonnegative(),
                textHash: hash,
              })
              .strict(),
          ),
        })
        .strict(),
    ),
  })
  .strict();
export const IndexArtifactV1 = z
  .object({
    schemaVersion: z.literal(1),
    recipe,
    recipeHash: hash,
    entries: z.array(entry),
    indexHash: hash,
  })
  .strict();

export type IndexArtifact = z.infer<typeof IndexArtifactV1>;
type IndexRecipe = z.infer<typeof recipe>;
type IndexEntry = z.infer<typeof entry>;

export type IndexCrashBoundary =
  | 'after-reservation'
  | 'after-entry-reservation'
  | 'after-entry'
  | 'before-artifact'
  | 'after-artifact'
  | 'before-terminal';

export class IndexAbruptInterruption extends Error {
  constructor() {
    super('index setup interrupted');
    this.name = 'IndexAbruptInterruption';
  }
}

interface IndexTerminal {
  readonly schemaVersion: 1;
  readonly attempt: number;
  readonly status: 'succeeded' | 'failed';
  readonly recipeHash: string;
  readonly indexHash: string | null;
  readonly usage: MeterUsage;
  readonly startedAt: string;
  readonly finishedAt: string;
}

export interface IndexBuildOptions {
  readonly root: string;
  readonly provider: LlmProvider;
  readonly meter: MeteredProvider;
  readonly metering: MeteringStore;
  readonly values?: readonly {
    readonly name: string;
    readonly description: string;
    readonly allowedTools: readonly string[];
    readonly indexText: string;
  }[];
  readonly embedder?: string;
  readonly dimensions?: number;
  readonly probes?: {
    readonly embedding: () => Promise<void>;
    readonly rerank: () => Promise<void>;
  };
  readonly crash?: (boundary: IndexCrashBoundary) => void | Promise<void>;
  readonly now?: () => Date;
}

type IndexSkill = NonNullable<IndexBuildOptions['values']>[number];

export interface IndexBuildResult {
  readonly artifact: IndexArtifact;
  readonly path: string;
  readonly reused: boolean;
  readonly attempt: number;
  readonly usage: MeterUsage;
}

const metadataText = (skill: {
  readonly name: string;
  readonly description: string;
  readonly allowedTools: readonly string[];
}): string =>
  `${skill.name} | ${skill.description} | ${skill.allowedTools.join(',')}`;

const textIdentity = (skillName: string, text: string) => ({
  skillName,
  text,
  textBytes: Buffer.byteLength(text, 'utf8'),
  textHash: artifactHash(text),
});

export const productionIndexRecipe = (
  values: readonly IndexSkill[] = mosaicSkills(),
  embedder: string = EMBEDDING_MODEL.model,
  dimensions: number = EMBEDDING_MODEL.dimensions,
): IndexRecipe => ({
  schemaVersion: 1,
  catalogHash: artifactHash(values),
  embedder,
  dimensions,
  algorithm: RETRIEVAL_ALGORITHM,
  views: [
    {
      view: 'body',
      entries: values.map((skill) => textIdentity(skill.name, skill.indexText)),
    },
    {
      view: 'metadata',
      entries: values.map((skill) =>
        textIdentity(skill.name, metadataText(skill)),
      ),
    },
  ],
});

const isMissing = (error: unknown): boolean =>
  typeof error === 'object' &&
  error !== null &&
  'code' in error &&
  error.code === 'ENOENT';

const isExisting = (error: unknown): boolean =>
  typeof error === 'object' &&
  error !== null &&
  'code' in error &&
  error.code === 'EEXIST';

const readJson = async <T>(path: string): Promise<T> =>
  JSON.parse(await readFile(path, 'utf8')) as T;

const writeExclusive = async (path: string, value: unknown): Promise<void> => {
  const handle = await open(path, 'wx', 0o600);
  try {
    await handle.writeFile(`${canonicalJson(value)}\n`, 'utf8');
    await handle.sync();
  } finally {
    await handle.close();
  }
};

const writeOrVerify = async (path: string, value: unknown): Promise<void> => {
  const content = `${canonicalJson(value)}\n`;
  try {
    await writeExclusive(path, value);
  } catch (error) {
    if (!isExisting(error) || (await readFile(path, 'utf8')) !== content) {
      throw new Error('immutable index artifact conflict');
    }
  }
};

const entryName = (
  view: IndexEntry['view'],
  index: number,
  textHash: string,
): string =>
  `${view}-${String(index + 1).padStart(3, '0')}-${textHash.slice(7)}.json`;

const validateEntry = (
  value: unknown,
  expected: IndexRecipe['views'][number]['entries'][number],
  view: IndexEntry['view'],
  dimensions: number,
): IndexEntry => {
  const parsed = entry.parse(value);
  if (
    parsed.view !== view ||
    parsed.skillName !== expected.skillName ||
    parsed.text !== expected.text ||
    parsed.textBytes !== expected.textBytes ||
    parsed.textHash !== expected.textHash ||
    parsed.vector.length !== dimensions
  ) {
    throw new Error('index entry differs from its content recipe');
  }
  return parsed;
};

const readEntries = async (
  directory: string,
  expected: IndexRecipe,
): Promise<Map<string, IndexEntry>> => {
  let names: readonly string[];
  try {
    names = (await readdir(directory)).filter((name) => name.endsWith('.json'));
  } catch (error) {
    if (isMissing(error)) return new Map();
    throw error;
  }
  const locations = new Map(
    expected.views.flatMap((view) =>
      view.entries.map(
        (item, index) =>
          [
            entryName(view.view, index, item.textHash),
            { expected: item, view: view.view },
          ] as const,
      ),
    ),
  );
  const values = new Map<string, IndexEntry>();
  for (const name of names) {
    const location = locations.get(name);
    if (location === undefined) throw new Error('unexpected index entry file');
    values.set(
      name,
      validateEntry(
        await readJson(join(directory, name)),
        location.expected,
        location.view,
        expected.dimensions,
      ),
    );
  }
  return values;
};

const artifactBody = (
  recipeValue: IndexRecipe,
  entries: readonly IndexEntry[],
) => ({
  schemaVersion: 1 as const,
  recipe: recipeValue,
  recipeHash: artifactHash(recipeValue),
  entries,
});

const loadTerminal = async (
  path: string,
): Promise<IndexTerminal | undefined> => {
  try {
    return await readJson<IndexTerminal>(path);
  } catch (error) {
    if (isMissing(error)) return undefined;
    throw error;
  }
};

/** Builds or resumes one immutable content-addressed retrieval index. */
export const buildProductionIndex = async (
  options: IndexBuildOptions,
): Promise<IndexBuildResult> => {
  const recipeValue = productionIndexRecipe(
    options.values,
    options.embedder,
    options.dimensions,
  );
  const recipeHash = artifactHash(recipeValue);
  const setup = join(options.root, 'setup', 'indexes', recipeHash.slice(7));
  const entriesDirectory = join(setup, 'entries');
  const callsDirectory = join(setup, 'calls');
  const attemptsDirectory = join(setup, 'attempts');
  await mkdir(entriesDirectory, { recursive: true });
  await mkdir(callsDirectory, { recursive: true });
  await mkdir(attemptsDirectory, { recursive: true });
  const names = (await readdir(attemptsDirectory))
    .filter((name) => /^attempt-[0-9]{3}$/u.test(name))
    .sort();
  let attempt = names.length;
  let attemptDirectory = names.at(-1)
    ? join(attemptsDirectory, names.at(-1)!)
    : undefined;
  let terminal =
    attemptDirectory === undefined
      ? undefined
      : await loadTerminal(join(attemptDirectory, 'terminal.json'));
  if (terminal?.status === 'succeeded' && terminal.indexHash !== null) {
    const path = join(
      options.root,
      'indexes',
      `${terminal.indexHash.slice(7)}.json`,
    );
    const artifact = await loadProductionIndex(path, recipeValue);
    return { artifact, path, reused: true, attempt, usage: terminal.usage };
  }
  if (attemptDirectory === undefined || terminal !== undefined) {
    attempt += 1;
    attemptDirectory = join(
      attemptsDirectory,
      `attempt-${String(attempt).padStart(3, '0')}`,
    );
    await mkdir(attemptDirectory);
    await writeExclusive(join(attemptDirectory, 'reservation.json'), {
      schemaVersion: 1,
      attempt,
      recipeHash,
      reservedAt: (options.now?.() ?? new Date()).toISOString(),
    });
  }
  const reservation = await readJson<{ readonly reservedAt: string }>(
    join(attemptDirectory, 'reservation.json'),
  );
  const scope = { id: `index-${recipeHash.slice(7)}`, attempt };
  options.meter.reset(await options.metering.usage(scope));
  options.meter.bind({
    append: (value) => options.metering.append(scope, value),
  });
  try {
    await options.crash?.('after-reservation');
    if (options.probes !== undefined) {
      options.meter.stage('probe');
      const journal = await options.metering.read(scope);
      for (const operation of ['embedding', 'rerank'] as const) {
        if (
          journal.some(
            (entry) => entry.stage === 'probe' && entry.operation === operation,
          )
        ) {
          continue;
        }
        const marker = join(callsDirectory, `probe-${operation}.started`);
        try {
          await access(marker);
          throw new Error(
            'unresolved paid capability probe requires audited recovery',
          );
        } catch (error) {
          if (!isMissing(error)) throw error;
        }
        await writeExclusive(marker, {
          schemaVersion: 1,
          attempt,
          recipeHash,
          operation,
        });
        await options.probes[operation]();
      }
    }
    options.meter.stage('index');
    const stored = await readEntries(entriesDirectory, recipeValue);
    const completed: IndexEntry[] = [];
    for (const view of recipeValue.views) {
      for (const [index, identity] of view.entries.entries()) {
        const name = entryName(view.view, index, identity.textHash);
        const prior = stored.get(name);
        if (prior !== undefined) {
          completed.push(prior);
          continue;
        }
        const callPath = join(callsDirectory, `${name}.started`);
        try {
          await access(callPath);
          throw new Error(
            'unresolved paid index call requires audited recovery',
          );
        } catch (error) {
          if (!isMissing(error)) throw error;
        }
        await writeExclusive(callPath, {
          schemaVersion: 1,
          attempt,
          recipeHash,
          view: view.view,
          skillName: identity.skillName,
          textHash: identity.textHash,
        });
        await options.crash?.('after-entry-reservation');
        const vector = await options.provider.embedding({
          model: recipeValue.embedder,
          input: identity.text,
          dimensions: recipeValue.dimensions,
          flags: { sensitiveOutput: true },
        });
        const value = validateEntry(
          { ...identity, view: view.view, vector },
          identity,
          view.view,
          recipeValue.dimensions,
        );
        await writeExclusive(join(entriesDirectory, name), value);
        completed.push(value);
        await options.crash?.('after-entry');
      }
    }
    await options.crash?.('before-artifact');
    const body = artifactBody(recipeValue, completed);
    const artifact = IndexArtifactV1.parse({
      ...body,
      indexHash: artifactHash(body),
    });
    const indexDirectory = join(options.root, 'indexes');
    await mkdir(indexDirectory, { recursive: true });
    const path = join(indexDirectory, `${artifact.indexHash.slice(7)}.json`);
    await writeOrVerify(path, artifact);
    await options.crash?.('after-artifact');
    await options.crash?.('before-terminal');
    const result = {
      schemaVersion: 1 as const,
      attempt,
      status: 'succeeded' as const,
      recipeHash,
      indexHash: artifact.indexHash,
      usage: options.meter.snapshot(),
      startedAt: reservation.reservedAt,
      finishedAt: (options.now?.() ?? new Date()).toISOString(),
    };
    await writeExclusive(join(attemptDirectory, 'terminal.json'), result);
    return { artifact, path, reused: false, attempt, usage: result.usage };
  } catch (error) {
    if (error instanceof IndexAbruptInterruption) throw error;
    const result = {
      schemaVersion: 1 as const,
      attempt,
      status: 'failed' as const,
      recipeHash,
      indexHash: null,
      usage: options.meter.snapshot(),
      startedAt: reservation.reservedAt,
      finishedAt: (options.now?.() ?? new Date()).toISOString(),
    };
    await writeOrVerify(join(attemptDirectory, 'terminal.json'), result);
    throw error;
  } finally {
    options.meter.bind(undefined);
  }
};

/** Loads an index only when its bytes, recipe, and content hash all agree. */
export const loadProductionIndex = async (
  path: string,
  expected = productionIndexRecipe(),
): Promise<IndexArtifact> => {
  const bytes = await readFile(path, 'utf8');
  const artifact = IndexArtifactV1.parse(JSON.parse(bytes) as unknown);
  if (bytes !== `${canonicalJson(artifact)}\n`) {
    throw new Error('retrieval index bytes are not canonical');
  }
  const body = artifactBody(artifact.recipe, artifact.entries);
  if (
    artifact.recipeHash !== artifactHash(artifact.recipe) ||
    artifact.indexHash !== artifactHash(body) ||
    artifactHash(artifact.recipe) !== artifactHash(expected)
  ) {
    throw new Error('retrieval index hash or recipe mismatch');
  }
  const expectedEntries = expected.views.flatMap((view) =>
    view.entries.map((identity) => ({ view: view.view, identity })),
  );
  if (artifact.entries.length !== expectedEntries.length) {
    throw new Error('retrieval index entry count mismatch');
  }
  artifact.entries.forEach((value, index) => {
    const target = expectedEntries[index];
    if (target === undefined) throw new Error('retrieval index order mismatch');
    validateEntry(value, target.identity, target.view, expected.dimensions);
  });
  if (basename(path) !== `${artifact.indexHash.slice(7)}.json`) {
    throw new Error('retrieval index filename hash mismatch');
  }
  return artifact;
};
