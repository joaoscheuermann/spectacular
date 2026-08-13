import { createHash } from 'node:crypto';
import {
  cp,
  lstat,
  mkdir,
  readdir,
  readFile,
  realpath,
  stat,
} from 'node:fs/promises';
import {
  dirname,
  isAbsolute,
  join,
  posix,
  relative,
  resolve,
  sep,
} from 'node:path';

import { SkillSchema, type Skill } from 'bundle';
import { parse } from 'yaml';

export const skillsbenchV1_1 = {
  repository: 'benchflow-ai/skillsbench',
  revision: 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af',
  tasksPath: 'tasks',
  taskCount: 87,
} as const;

export type SkillsbenchFileProvenance = {
  readonly path: string;
  readonly bytes: number;
  readonly sha256: string;
};

export type SkillsbenchSkillSource = {
  readonly taskId: string;
  readonly directory: string;
  readonly path: string;
};

export type SkillsbenchManifestSkill = {
  readonly id: string;
  readonly originalName: string;
  readonly description: string;
  readonly packageSha256: string;
  readonly files: readonly SkillsbenchFileProvenance[];
  readonly sources: readonly SkillsbenchSkillSource[];
};

export type SkillsbenchManifestTask = {
  readonly id: string;
  readonly goldSkillIds: readonly string[];
};

export type SkillsbenchCompositionManifest = {
  readonly schemaVersion: 1;
  readonly condition: 'skillsbench-composition';
  readonly source: typeof skillsbenchV1_1;
  readonly counts: {
    readonly tasks: number;
    readonly skillOccurrences: number;
    readonly catalogSkills: number;
  };
  readonly catalogSha256: string;
  readonly skills: readonly SkillsbenchManifestSkill[];
  readonly tasks: readonly SkillsbenchManifestTask[];
  readonly manifestSha256: string;
};

export type SkillsbenchCatalogSkill = {
  readonly id: string;
  readonly originalName: string;
  readonly packageSha256: string;
  readonly sources: readonly SkillsbenchSkillSource[];
  readonly skill: Skill;
};

export type SkillsbenchCatalog = {
  readonly manifest: SkillsbenchCompositionManifest;
  readonly skills: readonly SkillsbenchCatalogSkill[];
};

export type ScanSkillsbenchCatalogOptions = {
  readonly root: string;
  readonly revision: string;
};

type ScannedFile = SkillsbenchFileProvenance & {
  readonly data: Buffer;
};

type SkillOccurrence = {
  readonly taskId: string;
  readonly packageSha256: string;
  readonly files: readonly SkillsbenchFileProvenance[];
  readonly source: SkillsbenchSkillSource;
  readonly skill: Skill;
};

type SkillIdentity = {
  readonly key: string;
  readonly packageSha256: string;
  readonly files: readonly SkillsbenchFileProvenance[];
  readonly sources: readonly SkillsbenchSkillSource[];
  readonly skill: Skill;
};

const safeId = /^[a-z0-9][a-z0-9._-]*$/;

const compare = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;

const sha256 = (value: string | Buffer): string =>
  createHash('sha256').update(value).digest('hex');

const canonicalJson = (value: unknown): string => {
  if (value === null) return 'null';
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(',')}]`;
  if (typeof value === 'object') {
    const record = value as Readonly<Record<string, unknown>>;
    return `{${Object.keys(record)
      .sort(compare)
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(record[key])}`)
      .join(',')}}`;
  }
  const serialized = JSON.stringify(value);
  if (serialized === undefined) throw new TypeError('Value is not JSON data.');
  return serialized;
};

const digestJson = (domain: string, value: unknown): string =>
  sha256(`${domain}\n${canonicalJson(value)}`);

const splitFrontmatter = (
  source: string,
): { readonly metadata: string; readonly body: string } => {
  const match = /^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)([\s\S]*)$/.exec(source);
  if (match === null) throw new Error('Skill must contain YAML frontmatter.');
  return { metadata: match[1]!, body: match[2]! };
};

const parseFields = (source: string): Record<string, unknown> => {
  const value = parse(source) as unknown;
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error('Skill frontmatter must be an object.');
  }
  return value as Record<string, unknown>;
};

const definition = (source: string, path: string): Skill => {
  try {
    const { metadata, body } = splitFrontmatter(source);
    const fields = parseFields(metadata);
    const skill = SkillSchema.parse({
      name: fields.name,
      description: fields.description,
      body,
      allowedTools: ['terminal'],
    });
    if (!safeId.test(skill.name)) throw new Error('Unsafe skill name.');
    return skill;
  } catch {
    throw new Error(`Invalid skill definition: ${path}/SKILL.md`);
  }
};

const scanFiles = async (
  directory: string,
  prefix = '',
): Promise<readonly ScannedFile[]> => {
  const entries = (await readdir(directory, { withFileTypes: true })).sort(
    (left, right) => compare(left.name, right.name),
  );
  const files = await Promise.all(
    entries.map(async (entry): Promise<readonly ScannedFile[]> => {
      const path = prefix === '' ? entry.name : posix.join(prefix, entry.name);
      const target = join(directory, entry.name);
      if (entry.isSymbolicLink())
        throw new Error(
          `Symbolic links are not allowed in skill package: ${path}`,
        );
      if (entry.isDirectory()) return scanFiles(target, path);
      if (!entry.isFile())
        throw new Error(`Unsupported skill package entry: ${path}`);
      const data = await readFile(target);
      return [{ path, bytes: data.byteLength, sha256: sha256(data), data }];
    }),
  );
  return files.flat().sort((left, right) => compare(left.path, right.path));
};

const sourcePath = (taskId: string, directory: string): string =>
  posix.join(
    skillsbenchV1_1.tasksPath,
    taskId,
    'environment',
    'skills',
    directory,
  );

const scanPackage = async (
  root: string,
  taskId: string,
  directory: string,
): Promise<SkillOccurrence> => {
  const path = sourcePath(taskId, directory);
  const scanned = await scanFiles(join(root, ...path.split('/')));
  const skillFile = scanned.find((file) => file.path === 'SKILL.md');
  if (skillFile === undefined)
    throw new Error(`Missing skill definition: ${path}`);
  const files = scanned.map(({ data: _data, ...file }) => file);
  return {
    taskId,
    files,
    packageSha256: digestJson('skillsbench-package-v1', files),
    source: { taskId, directory, path },
    skill: definition(skillFile.data.toString('utf8'), path),
  };
};

const hasSkill = async (directory: string): Promise<boolean> => {
  try {
    const value = await lstat(join(directory, 'SKILL.md'));
    if (value.isSymbolicLink())
      throw new Error('Symbolic skill definitions are not allowed.');
    return value.isFile();
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false;
    throw error;
  }
};

const scanTask = async (
  root: string,
  taskId: string,
): Promise<readonly SkillOccurrence[]> => {
  if (!safeId.test(taskId)) throw new Error(`Unsafe task ID: ${taskId}`);
  const taskRoot = join(root, skillsbenchV1_1.tasksPath, taskId);
  if (!(await stat(join(taskRoot, 'task.md'))).isFile())
    throw new Error(`Missing task definition: ${taskId}/task.md`);
  const skillsRoot = join(taskRoot, 'environment', 'skills');
  const entries = (await readdir(skillsRoot, { withFileTypes: true })).sort(
    (left, right) => compare(left.name, right.name),
  );
  if (entries.some((entry) => entry.isSymbolicLink()))
    throw new Error(`Symbolic skill packages are not allowed: ${taskId}`);
  const directories = entries.filter((entry) => entry.isDirectory());
  const candidates = await Promise.all(
    directories.map(async (entry) => ({
      directory: entry.name,
      valid: await hasSkill(join(skillsRoot, entry.name)),
    })),
  );
  const packages = await Promise.all(
    candidates
      .filter((candidate) => candidate.valid)
      .map((candidate) => scanPackage(root, taskId, candidate.directory)),
  );
  if (packages.length === 0) throw new Error(`Task has no skills: ${taskId}`);
  return packages;
};

const taskIds = async (root: string): Promise<readonly string[]> => {
  const directory = join(root, skillsbenchV1_1.tasksPath);
  const entries = (await readdir(directory, { withFileTypes: true })).sort(
    (left, right) => compare(left.name, right.name),
  );
  if (entries.some((entry) => !entry.isDirectory()))
    throw new Error('SkillsBench tasks must be direct directories.');
  const ids = entries.map((entry) => entry.name);
  if (ids.length !== skillsbenchV1_1.taskCount) {
    throw new Error(
      `SkillsBench v1.1 requires exactly ${skillsbenchV1_1.taskCount} task directories.`,
    );
  }
  return ids;
};

const identities = (
  occurrences: readonly SkillOccurrence[],
): readonly SkillIdentity[] => {
  const values = new Map<string, SkillIdentity>();
  for (const occurrence of occurrences) {
    const key = `${occurrence.skill.name}\u0000${occurrence.packageSha256}`;
    const current = values.get(key);
    values.set(key, {
      key,
      packageSha256: occurrence.packageSha256,
      files: occurrence.files,
      sources: [...(current?.sources ?? []), occurrence.source].sort((a, b) =>
        compare(a.path, b.path),
      ),
      skill: occurrence.skill,
    });
  }
  return [...values.values()].sort(
    (left, right) =>
      compare(left.skill.name, right.skill.name) ||
      compare(left.packageSha256, right.packageSha256),
  );
};

const assignIds = (
  values: readonly SkillIdentity[],
): readonly (SkillIdentity & { readonly id: string })[] => {
  const counts = new Map<string, number>();
  for (const value of values)
    counts.set(value.skill.name, (counts.get(value.skill.name) ?? 0) + 1);
  const assigned = values.map((value) => ({
    ...value,
    id:
      counts.get(value.skill.name) === 1
        ? value.skill.name
        : `skillsbench--${value.skill.name}--${value.packageSha256}`,
  }));
  if (new Set(assigned.map((value) => value.id)).size !== assigned.length)
    throw new Error('Catalog skill IDs are not unique.');
  if (assigned.some((value) => !safeId.test(value.id)))
    throw new Error('Catalog skill ID is not safe.');
  return assigned;
};

const manifestSkills = (
  values: readonly (SkillIdentity & { readonly id: string })[],
): readonly SkillsbenchManifestSkill[] =>
  values.map((value) => ({
    id: value.id,
    originalName: value.skill.name,
    description: value.skill.description,
    packageSha256: value.packageSha256,
    files: value.files,
    sources: value.sources,
  }));

const manifestTasks = (
  ids: readonly string[],
  occurrences: readonly SkillOccurrence[],
  catalog: readonly (SkillIdentity & { readonly id: string })[],
): readonly SkillsbenchManifestTask[] => {
  const catalogIds = new Map(catalog.map((value) => [value.key, value.id]));
  return ids.map((id) => {
    const goldSkillIds = occurrences
      .filter((occurrence) => occurrence.taskId === id)
      .map((occurrence) =>
        catalogIds.get(
          `${occurrence.skill.name}\u0000${occurrence.packageSha256}`,
        ),
      )
      .filter((value): value is string => value !== undefined)
      .sort(compare);
    if (new Set(goldSkillIds).size !== goldSkillIds.length)
      throw new Error(`Task contains duplicate skill packages: ${id}`);
    return { id, goldSkillIds };
  });
};

/** Scans a caller-verified local SkillsBench v1.1 checkout without network access. */
export const scanSkillsbenchCatalog = async (
  options: ScanSkillsbenchCatalogOptions,
): Promise<SkillsbenchCatalog> => {
  if (options.revision !== skillsbenchV1_1.revision)
    throw new Error(
      'Catalog scan requires the pinned SkillsBench v1.1 revision.',
    );
  const root = resolve(options.root);
  const ids = await taskIds(root);
  const occurrences = (await Promise.all(ids.map((id) => scanTask(root, id))))
    .flat()
    .sort((left, right) => compare(left.source.path, right.source.path));
  const catalog = assignIds(identities(occurrences));
  const skills = manifestSkills(catalog);
  const tasks = manifestTasks(ids, occurrences, catalog);
  const counts = {
    tasks: ids.length,
    skillOccurrences: occurrences.length,
    catalogSkills: catalog.length,
  };
  const catalogSha256 = digestJson(
    'skillsbench-catalog-v1',
    skills.map(({ sources: _sources, ...entry }) => entry),
  );
  const unsigned = {
    schemaVersion: 1 as const,
    condition: 'skillsbench-composition' as const,
    source: skillsbenchV1_1,
    counts,
    catalogSha256,
    skills,
    tasks,
  };
  const manifest = {
    ...unsigned,
    manifestSha256: digestJson('skillsbench-composition-manifest-v1', unsigned),
  };

  return {
    manifest,
    skills: catalog.map((value) => ({
      id: value.id,
      originalName: value.skill.name,
      packageSha256: value.packageSha256,
      sources: value.sources,
      skill: SkillSchema.parse({
        name: value.id,
        description: value.skill.description,
        body: value.skill.body,
        allowedTools: value.skill.allowedTools,
      }),
    })),
  };
};

/** Serializes a manifest deterministically for persisted campaign evidence. */
export const stringifySkillsbenchCompositionManifest = (
  manifest: SkillsbenchCompositionManifest,
): string => `${JSON.stringify(manifest, null, 2)}\n`;

const canonicalSource = async (
  root: string,
  entry: SkillsbenchCatalogSkill,
): Promise<string> => {
  const source = [...entry.sources].sort((left, right) =>
    compare(left.path, right.path),
  )[0];
  if (source === undefined)
    throw new Error(`Catalog skill has no source: ${entry.id}`);
  const canonicalRoot = await realpath(resolve(root));
  const directory = await realpath(
    join(canonicalRoot, ...source.path.split('/')),
  );
  const local = relative(canonicalRoot, directory);
  if (local === '..' || local.startsWith(`..${sep}`) || isAbsolute(local))
    throw new Error(`Catalog skill source escapes checkout: ${entry.id}`);
  return directory;
};

/**
 * Copies packages into task-neutral catalog paths and materializes runtime bodies.
 * The fresh neutral root must be outside the upstream checkout so task provenance
 * cannot leak through model-visible package paths.
 */
export const materializeSkillsbenchCatalog = async (
  catalog: SkillsbenchCatalog,
  root: string,
  neutralRoot: string,
): Promise<readonly Skill[]> => {
  const canonicalRoot = await realpath(resolve(root));
  const targetRoot = resolve(neutralRoot);
  const local = relative(canonicalRoot, targetRoot);
  if (local !== '..' && !local.startsWith(`..${sep}`) && !isAbsolute(local)) {
    throw new Error(
      'Neutral catalog root must be outside the upstream checkout.',
    );
  }
  try {
    await lstat(targetRoot);
    throw new Error(`Neutral catalog root already exists: ${targetRoot}`);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error;
  }
  const sources = await Promise.all(
    catalog.skills.map(async (entry) => ({
      entry,
      source: await canonicalSource(canonicalRoot, entry),
      target: join(targetRoot, entry.id),
    })),
  );
  await mkdir(dirname(targetRoot), { recursive: true });
  await mkdir(targetRoot);
  const materialized: Skill[] = [];
  for (const { entry, source, target } of sources) {
    await cp(source, target, {
      recursive: true,
      errorOnExist: true,
      force: false,
    });
    materialized.push(
      SkillSchema.parse({
        name: entry.skill.name,
        description: entry.skill.description,
        body: `Skill files are in ${target}. Resolve scripts/, references/, and other relative paths from this directory.\n\n${entry.skill.body}`,
        allowedTools: entry.skill.allowedTools,
      }),
    );
  }
  return materialized;
};
