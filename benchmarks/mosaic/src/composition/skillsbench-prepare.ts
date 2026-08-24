import { execFile } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import {
  link,
  mkdir,
  open,
  readFile,
  realpath,
  unlink,
} from 'node:fs/promises';
import { basename, dirname, join, resolve } from 'node:path';
import { promisify } from 'node:util';

import {
  defineSkillsbenchComposition,
  type SkillsbenchCompositionContract,
  type SkillsbenchFixedRankingInput,
} from './skillsbench-arms.js';
import {
  scanSkillsbenchCatalog,
  skillsbenchV1_1,
  type SkillsbenchCatalog,
  type SkillsbenchCompositionManifest,
} from './skillsbench-catalog.js';

export const skillsbenchLexicalRanker = {
  id: 'tfidf-unigram-bigram-cosine',
  revision: 'skillsbench-composition-ranker-v1',
} as const;

/** Expected bytes-derived identity for the canonical LF checkout at the pin. */
export const skillsbenchCompositionPin = {
  skillOccurrences: 232,
  catalogSkills: 209,
  catalogSha256:
    '382379cc8b2ac56aab6d6c4559bb2e6b1d6203f5de58534532e9ad528bdefe7c',
  rankingSha256:
    '0a1631e7ad74730c909194efee941d2ba981279cd8d040941d2a5039fafc9ffd',
} as const;

export interface SkillsbenchPreparation {
  readonly schemaVersion: 1;
  readonly benchmark: 'SkillsBench Composition';
  readonly catalogManifest: SkillsbenchCompositionManifest;
  readonly contract: SkillsbenchCompositionContract;
}

export interface SkillsbenchCommandRunner {
  readonly run: (
    command: string,
    args: readonly string[],
  ) => Promise<{ readonly stdout: string }>;
}

export interface PrepareSkillsbenchCompositionOptions {
  readonly sourceRoot: string;
  readonly runner?: SkillsbenchCommandRunner;
}

type ReadTask = (taskId: string) => Promise<string>;

/** Verifies exact Git identity and a byte-clean task tree before scanning. */
export const verifySkillsbenchCheckout = async (
  sourceRoot: string,
  runner: SkillsbenchCommandRunner = defaultRunner,
): Promise<string> => {
  const root = await realpath(resolve(sourceRoot));
  const top = await git(runner, root, ['rev-parse', '--show-toplevel']);
  if ((await realpath(resolve(top.trim()))) !== root)
    throw new Error('SkillsBench source must be the checkout root.');
  const head = await git(runner, root, ['rev-parse', 'HEAD']);
  if (head.trim() !== skillsbenchV1_1.revision)
    throw new Error('SkillsBench checkout HEAD does not match the v1.1 pin.');
  const dirty = await git(runner, root, [
    'status',
    '--porcelain=v1',
    '--untracked-files=all',
    '--ignored',
    '--',
    skillsbenchV1_1.tasksPath,
  ]);
  if (dirty.trim() !== '')
    throw new Error(
      'SkillsBench tasks tree must be clean, including ignored files.',
    );
  const staged = await git(runner, root, [
    'ls-files',
    '--stage',
    '--',
    ':(glob)tasks/*/environment/skills/**',
  ]);
  if (staged.split('\n').some((line) => /^(?:120000|160000)\s/u.test(line))) {
    throw new Error(
      'SkillsBench skill packages must not contain Git symlinks or submodules.',
    );
  }
  return root;
};

/** Builds a complete deterministic catalog permutation for every task. */
export const buildSkillsbenchFixedRanking = async (
  catalog: SkillsbenchCatalog,
  readTask: ReadTask,
): Promise<SkillsbenchFixedRankingInput> => {
  const skills = catalog.skills.map((entry) => ({
    id: entry.id,
    text: [entry.originalName, entry.skill.description, entry.skill.body].join(
      '\n',
    ),
  }));
  if (skills.length === 0)
    throw new Error('SkillsBench ranking requires a non-empty catalog.');
  const documents = skills.map(({ id, text }) => ({
    id,
    terms: termCounts(text),
  }));
  const idf = inverseDocumentFrequencies(documents.map(({ terms }) => terms));
  const documentVectors = documents.map(({ id, terms }) => ({
    id,
    vector: tfidf(terms, idf),
  }));
  const tasks = [];
  for (const task of catalog.manifest.tasks) {
    const query = tfidf(termCounts(await readTask(task.id)), idf);
    const skillIds = documentVectors
      .map(({ id, vector }) => ({ id, score: cosine(query, vector) }))
      .sort(
        (left, right) => right.score - left.score || compare(left.id, right.id),
      )
      .map(({ id }) => id);
    tasks.push({ id: task.id, skillIds });
  }
  return {
    catalogSha256: catalog.manifest.catalogSha256,
    ranker: skillsbenchLexicalRanker,
    tasks,
  };
};

/** Creates the verified offline catalog, ranking, and five-arm contract. */
export const prepareSkillsbenchComposition = async (
  options: PrepareSkillsbenchCompositionOptions,
): Promise<SkillsbenchPreparation> => {
  const root = await verifySkillsbenchCheckout(
    options.sourceRoot,
    options.runner,
  );
  const catalog = await scanSkillsbenchCatalog({
    root,
    revision: skillsbenchV1_1.revision,
  });
  if (
    catalog.manifest.counts.skillOccurrences !==
      skillsbenchCompositionPin.skillOccurrences ||
    catalog.manifest.counts.catalogSkills !==
      skillsbenchCompositionPin.catalogSkills ||
    catalog.manifest.catalogSha256 !== skillsbenchCompositionPin.catalogSha256
  ) {
    throw new Error(
      'SkillsBench catalog bytes do not match the pinned composition identity.',
    );
  }
  const ranking = await buildSkillsbenchFixedRanking(catalog, (taskId) =>
    readFile(join(root, skillsbenchV1_1.tasksPath, taskId, 'task.md'), 'utf8'),
  );
  const contract = defineSkillsbenchComposition(catalog.manifest, ranking);
  if (contract.fixedRanking.sha256 !== skillsbenchCompositionPin.rankingSha256)
    throw new Error(
      'SkillsBench task text or lexical ranking does not match its pin.',
    );
  return {
    schemaVersion: 1,
    benchmark: 'SkillsBench Composition',
    catalogManifest: catalog.manifest,
    contract,
  };
};

/** Writes one fully prepared artifact without replacing existing evidence. */
export const writeSkillsbenchPreparation = async (
  outputPath: string,
  preparation: SkillsbenchPreparation,
): Promise<void> => {
  const path = resolve(outputPath);
  await mkdir(dirname(path), { recursive: true });
  const temporary = join(
    dirname(path),
    `.${basename(path)}.${randomUUID()}.tmp`,
  );
  try {
    const handle = await open(temporary, 'wx');
    try {
      await handle.writeFile(
        `${JSON.stringify(preparation, null, 2)}\n`,
        'utf8',
      );
      await handle.sync();
    } finally {
      await handle.close();
    }
    await link(temporary, path);
  } finally {
    try {
      await unlink(temporary);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
    }
  }
};

const exec = promisify(execFile);
const defaultRunner: SkillsbenchCommandRunner = {
  run: async (command, args) => {
    const result = await exec(command, [...args], {
      encoding: 'utf8',
      windowsHide: true,
      maxBuffer: 16 * 1024 * 1024,
    });
    return { stdout: result.stdout };
  },
};

const git = async (
  runner: SkillsbenchCommandRunner,
  root: string,
  args: readonly string[],
): Promise<string> => {
  try {
    return (await runner.run('git', ['-C', root, ...args])).stdout;
  } catch {
    throw new Error('Cannot verify the pinned SkillsBench Git checkout.');
  }
};

type Counts = ReadonlyMap<string, number>;
type Vector = ReadonlyMap<string, number>;

const tokens = (value: string): readonly string[] =>
  [
    ...value
      .normalize('NFKC')
      .toLowerCase()
      .matchAll(/[\p{L}\p{N}]+/gu),
  ].map(([token]) => token);

const features = (value: string): readonly string[] => {
  const values = tokens(value);
  return [
    ...values.map((token) => `u:${token}`),
    ...values
      .slice(0, -1)
      .map((token, index) => `b:${token}\u0000${values[index + 1]}`),
  ];
};

const termCounts = (value: string): Counts => {
  const counts = new Map<string, number>();
  features(value).forEach((term) =>
    counts.set(term, (counts.get(term) ?? 0) + 1),
  );
  return counts;
};

const inverseDocumentFrequencies = (documents: readonly Counts[]): Vector => {
  const frequencies = new Map<string, number>();
  documents.forEach((document) =>
    document.forEach((_, term) =>
      frequencies.set(term, (frequencies.get(term) ?? 0) + 1),
    ),
  );
  return new Map(
    [...frequencies].map(([term, frequency]) => [
      term,
      Math.log((documents.length + 1) / (frequency + 1)) + 1,
    ]),
  );
};

const tfidf = (counts: Counts, idf: Vector): Vector => {
  const total = [...counts.values()].reduce((sum, count) => sum + count, 0);
  if (total === 0) return new Map();
  return new Map(
    [...counts].flatMap(([term, count]) => {
      const inverse = idf.get(term);
      return inverse === undefined
        ? []
        : [[term, (count / total) * inverse] as const];
    }),
  );
};

const cosine = (left: Vector, right: Vector): number => {
  const leftNorm = norm(left);
  const rightNorm = norm(right);
  if (leftNorm === 0 || rightNorm === 0) return 0;
  let dot = 0;
  left.forEach((value, term) => {
    dot += value * (right.get(term) ?? 0);
  });
  return dot / (leftNorm * rightNorm);
};

const norm = (value: Vector): number =>
  Math.sqrt([...value.values()].reduce((sum, child) => sum + child ** 2, 0));

const compare = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;
