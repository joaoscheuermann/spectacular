import { readdir, readFile, stat } from 'node:fs/promises';
import { register } from 'node:module';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import { parse } from 'yaml';

import { SkillSchema } from '../schemas/skill/index.js';
import { ToolDescriptorSchema } from '../schemas/tool/index.js';
import type { Skill } from '../types/skill.js';
import type { ToolDescriptor } from '../types/tool.js';

type Loaded<T> = {
  readonly bundle: string;
  readonly path: string;
  readonly value: T;
};

const compare = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

const withCause = (message: string, cause: unknown): Error =>
  new Error(message, { cause });

const isMissing = (cause: unknown): boolean =>
  isRecord(cause) && cause.code === 'ENOENT';

const equal = (left: unknown, right: unknown): boolean => {
  if (Object.is(left, right)) {
    return true;
  }

  if (Array.isArray(left) || Array.isArray(right)) {
    return (
      Array.isArray(left) &&
      Array.isArray(right) &&
      left.length === right.length &&
      left.every((value, index) => equal(value, right[index]))
    );
  }

  if (!isRecord(left) || !isRecord(right)) {
    return false;
  }

  const leftKeys = Object.keys(left).sort(compare);
  const rightKeys = Object.keys(right).sort(compare);

  return (
    leftKeys.length === rightKeys.length &&
    leftKeys.every(
      (key, index) => key === rightKeys[index] && equal(left[key], right[key]),
    )
  );
};

const context = (entry: Loaded<unknown>): string =>
  `"${entry.path}" in bundle "${entry.bundle}"`;

let typescriptLoaderRegistered = false;

const ensureTypescriptLoader = (): void => {
  if (typescriptLoaderRegistered) {
    return;
  }

  register('@swc-node/register/esm', import.meta.url);
  typescriptLoaderRegistered = true;
};

const readManifest = async (
  bundlePath: string,
): Promise<string | undefined> => {
  try {
    return await readFile(join(bundlePath, 'manifest.json'), 'utf8');
  } catch (cause) {
    if (isMissing(cause)) {
      return undefined;
    }

    throw withCause(
      `Unable to read manifest "manifest.json" in bundle "${bundlePath}".`,
      cause,
    );
  }
};

const validateManifest = (source: string, bundlePath: string): void => {
  let manifest: unknown;

  try {
    manifest = JSON.parse(source);
  } catch (cause) {
    throw withCause(
      `Unable to parse manifest "manifest.json" in bundle "${bundlePath}".`,
      cause,
    );
  }

  if (!isRecord(manifest)) {
    throw new Error(
      `Manifest "manifest.json" in bundle "${bundlePath}" must be a JSON object.`,
    );
  }
};

const discoverBundles = async (rootPath: string): Promise<string[]> => {
  let entries;

  try {
    entries = await readdir(rootPath, { withFileTypes: true });
  } catch (cause) {
    throw withCause(`Unable to discover bundles in root "${rootPath}".`, cause);
  }

  const directories = entries
    .filter((entry) => entry.isDirectory())
    .sort((left, right) => compare(left.name, right.name));
  const paths: string[] = [];

  for (const directory of directories) {
    const bundlePath = join(rootPath, directory.name);
    const source = await readManifest(bundlePath);

    if (source === undefined) {
      continue;
    }

    validateManifest(source, bundlePath);
    paths.push(bundlePath);
  }

  return paths;
};

const ensureDirectory = async (
  bundlePath: string,
  directoryPath: string,
  relativePath: string,
): Promise<void> => {
  let information;

  try {
    information = await stat(directoryPath);
  } catch (cause) {
    throw withCause(
      `Unable to access directory "${relativePath}" in bundle "${bundlePath}".`,
      cause,
    );
  }

  if (!information.isDirectory()) {
    throw new Error(
      `Expected "${relativePath}" in bundle "${bundlePath}" to be a directory.`,
    );
  }
};

const discoverSkills = async (bundlePath: string): Promise<string[]> => {
  const directoryPath = join(bundlePath, 'skills');
  await ensureDirectory(bundlePath, directoryPath, 'skills');

  try {
    const entries = await readdir(directoryPath, { withFileTypes: true });

    return entries
      .filter((entry) => entry.isDirectory())
      .map((entry) => `skills/${entry.name}/SKILL.md`)
      .sort(compare);
  } catch (cause) {
    throw withCause(
      `Unable to discover skills in bundle "${bundlePath}".`,
      cause,
    );
  }
};

const discoverTools = async (bundlePath: string): Promise<string[]> => {
  const directoryPath = join(bundlePath, 'tools');
  await ensureDirectory(bundlePath, directoryPath, 'tools');

  try {
    const entries = await readdir(directoryPath, { withFileTypes: true });

    return entries
      .filter(
        (entry) =>
          entry.isFile() &&
          (entry.name.endsWith('.ts') || entry.name.endsWith('.js')),
      )
      .map((entry) => `tools/${entry.name}`)
      .sort(compare);
  } catch (cause) {
    throw withCause(
      `Unable to discover tools in bundle "${bundlePath}".`,
      cause,
    );
  }
};

const splitSkill = (
  source: string,
  bundlePath: string,
  relativePath: string,
): { readonly metadata: string; readonly body: string } => {
  const opening = source.startsWith('---\r\n')
    ? '---\r\n'
    : source.startsWith('---\n')
      ? '---\n'
      : undefined;

  if (opening === undefined) {
    throw new Error(
      `Skill "${relativePath}" in bundle "${bundlePath}" must begin with a YAML frontmatter delimiter.`,
    );
  }

  const content = source.slice(opening.length);
  const closing = /^---(?:\r?\n|$)/m.exec(content);

  if (closing === null) {
    throw new Error(
      `Skill "${relativePath}" in bundle "${bundlePath}" has no closing YAML frontmatter delimiter.`,
    );
  }

  return {
    metadata: content.slice(0, closing.index),
    body: content.slice(closing.index + closing[0].length),
  };
};

const loadSkill = async (
  bundlePath: string,
  relativePath: string,
): Promise<Loaded<Skill>> => {
  const filePath = join(bundlePath, ...relativePath.split('/'));
  let source: string;

  try {
    source = await readFile(filePath, 'utf8');
  } catch (cause) {
    throw withCause(
      `Unable to read skill "${relativePath}" in bundle "${bundlePath}".`,
      cause,
    );
  }

  const { metadata: yaml, body } = splitSkill(source, bundlePath, relativePath);
  let metadata: unknown;

  try {
    metadata = parse(yaml);
  } catch (cause) {
    throw withCause(
      `Unable to parse YAML for skill "${relativePath}" in bundle "${bundlePath}".`,
      cause,
    );
  }

  if (!isRecord(metadata)) {
    throw new Error(
      `Skill "${relativePath}" in bundle "${bundlePath}" must have object YAML frontmatter.`,
    );
  }

  const { ['allowed-tools']: allowedTools, ...fields } = metadata;
  const input = Object.hasOwn(metadata, 'allowed-tools')
    ? { ...fields, allowedTools, body }
    : { ...fields, body };

  try {
    return {
      bundle: bundlePath,
      path: relativePath,
      value: SkillSchema.parse(input),
    };
  } catch (cause) {
    throw withCause(
      `Invalid skill "${relativePath}" in bundle "${bundlePath}".`,
      cause,
    );
  }
};

const loadTool = async (
  bundlePath: string,
  relativePath: string,
): Promise<Loaded<ToolDescriptor>> => {
  const filePath = join(bundlePath, ...relativePath.split('/'));
  let imported: unknown;

  if (relativePath.endsWith('.ts')) {
    try {
      ensureTypescriptLoader();
    } catch (cause) {
      throw withCause(
        `Unable to register the TypeScript loader for tool "${relativePath}" in bundle "${bundlePath}".`,
        cause,
      );
    }
  }

  try {
    imported = await import(pathToFileURL(filePath).href);
  } catch (cause) {
    throw withCause(
      `Unable to import tool "${relativePath}" in bundle "${bundlePath}".`,
      cause,
    );
  }

  if (!isRecord(imported) || !isRecord(imported.default)) {
    throw new Error(
      `Tool "${relativePath}" in bundle "${bundlePath}" must have an object default export.`,
    );
  }

  const definition = imported.default.definition;

  if (!isRecord(definition)) {
    throw new Error(
      `Tool "${relativePath}" in bundle "${bundlePath}" must expose a definition on its default export.`,
    );
  }

  const input = Object.hasOwn(definition, 'outputSchema')
    ? definition
    : { ...definition, outputSchema: {} };

  try {
    return {
      bundle: bundlePath,
      path: relativePath,
      value: ToolDescriptorSchema.parse(input),
    };
  } catch (cause) {
    throw withCause(
      `Invalid tool definition in "${relativePath}" from bundle "${bundlePath}".`,
      cause,
    );
  }
};

const rejectDuplicates = <T extends { readonly name: string }>(
  kind: 'skill' | 'tool',
  entries: readonly Loaded<T>[],
): void => {
  const resources = new Map<string, Loaded<T>>();

  for (const entry of entries) {
    const previous = resources.get(entry.value.name);

    if (previous !== undefined) {
      throw new Error(
        `Duplicate ${kind} name "${entry.value.name}": ` +
          `${context(previous)} and ${context(entry)}.`,
      );
    }

    resources.set(entry.value.name, entry);
  }
};

const rejectUnresolvedTools = (
  skills: readonly Loaded<Skill>[],
  tools: readonly Loaded<ToolDescriptor>[],
  bundlePath: string,
): void => {
  const names = new Set(tools.map((tool) => tool.value.name));

  for (const skill of skills) {
    for (const tool of skill.value.allowedTools) {
      if (!names.has(tool)) {
        throw new Error(
          `Skill "${skill.value.name}" in "${skill.path}" from bundle "${bundlePath}" ` +
            `references unavailable tool "${tool}".`,
        );
      }
    }
  }
};

const loadBundle = async (
  bundlePath: string,
): Promise<{
  readonly skills: Loaded<Skill>[];
  readonly tools: Loaded<ToolDescriptor>[];
}> => {
  const skillPaths = await discoverSkills(bundlePath);
  const toolPaths = await discoverTools(bundlePath);
  const skills: Loaded<Skill>[] = [];
  const tools: Loaded<ToolDescriptor>[] = [];

  for (const relativePath of skillPaths) {
    skills.push(await loadSkill(bundlePath, relativePath));
  }

  for (const relativePath of toolPaths) {
    tools.push(await loadTool(bundlePath, relativePath));
  }

  rejectDuplicates('skill', skills);
  rejectDuplicates('tool', tools);
  rejectUnresolvedTools(skills, tools, bundlePath);

  return { skills, tools };
};

type Aggregate = {
  readonly skills: Loaded<Skill>[];
  readonly tools: Loaded<ToolDescriptor>[];
  readonly skillNames: Map<string, Loaded<Skill>>;
  readonly toolNames: Map<string, Loaded<ToolDescriptor>>;
};

const addSkill = (aggregate: Aggregate, skill: Loaded<Skill>): void => {
  const previous = aggregate.skillNames.get(skill.value.name);

  if (previous !== undefined) {
    throw new Error(
      `Duplicate skill name "${skill.value.name}": ` +
        `${context(previous)} and ${context(skill)}.`,
    );
  }

  aggregate.skillNames.set(skill.value.name, skill);
  aggregate.skills.push(skill);
};

const addTool = (aggregate: Aggregate, tool: Loaded<ToolDescriptor>): void => {
  const previous = aggregate.toolNames.get(tool.value.name);

  if (previous === undefined) {
    aggregate.toolNames.set(tool.value.name, tool);
    aggregate.tools.push(tool);
    return;
  }

  if (!equal(previous.value, tool.value)) {
    throw new Error(
      `Conflicting tool name "${tool.value.name}": ` +
        `${context(previous)} and ${context(tool)}.`,
    );
  }
};

/**
 * Loads all manifest-marked bundles immediately below a bundle root.
 */
export async function bundles(
  path: string,
): Promise<{ skills: Skill[]; tools: ToolDescriptor[] }> {
  const rootPath = resolve(path);
  const bundlePaths = await discoverBundles(rootPath);
  const aggregate: Aggregate = {
    skills: [],
    tools: [],
    skillNames: new Map(),
    toolNames: new Map(),
  };

  for (const bundlePath of bundlePaths) {
    const bundle = await loadBundle(bundlePath);

    for (const skill of bundle.skills) {
      addSkill(aggregate, skill);
    }

    for (const tool of bundle.tools) {
      addTool(aggregate, tool);
    }
  }

  return {
    skills: aggregate.skills.map((skill) => skill.value),
    tools: aggregate.tools.map((tool) => tool.value),
  };
}
