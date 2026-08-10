import {
  access,
  constants,
  readdir,
  readFile,
  realpath,
  stat,
} from 'node:fs/promises';
import { join } from 'node:path';
import { SkillSchema, type Skill } from 'bundle';
import { parse } from 'yaml';

const skillDirectory = (home: string): string =>
  join(home, '.agents', 'skills');

const splitFrontmatter = (
  source: string,
): {
  readonly metadata: string;
  readonly body: string;
} => {
  const match = /^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)([\s\S]*)$/.exec(source);

  if (match === null) throw new Error('Skill must contain YAML frontmatter.');

  return { metadata: match[1], body: match[2] };
};

const frontmatter = (source: string): Record<string, unknown> => {
  const value = parse(source) as unknown;

  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error('Skill frontmatter must be an object.');
  }

  return value as Record<string, unknown>;
};

const load = async (directory: string): Promise<Skill> => {
  const canonicalDirectory = await realpath(directory);
  const source = await readFile(join(canonicalDirectory, 'SKILL.md'), 'utf8');
  const { metadata, body } = splitFrontmatter(source);
  const fields = frontmatter(metadata);

  return SkillSchema.parse({
    name: fields.name,
    description: fields.description,
    body: `Skill files are in ${canonicalDirectory}. Resolve scripts/, references/, and other relative paths from this directory.\n\n${body}`,
    allowedTools: ['terminal'],
  });
};

const hasSkill = async (directory: string): Promise<boolean> => {
  const path = join(directory, 'SKILL.md');

  try {
    if (!(await stat(path)).isFile()) return false;
    await access(path, constants.R_OK);
    return true;
  } catch {
    return false;
  }
};

const directories = async (home: string): Promise<readonly string[]> => {
  try {
    const entries = await readdir(skillDirectory(home), {
      withFileTypes: true,
    });
    const candidates = entries
      .filter((entry) => entry.isDirectory())
      .map((entry) => join(skillDirectory(home), entry.name));

    return (await Promise.all(candidates.map(hasSkill))).flatMap(
      (valid, index) => (valid ? [candidates[index]!] : []),
    );
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return [];
    throw error;
  }
};

const rejectDuplicates = (skills: readonly Skill[]): readonly Skill[] => {
  const duplicate = skills.find(
    (skill, index) => index > 0 && skill.name === skills[index - 1]?.name,
  );

  if (duplicate !== undefined) {
    throw new Error(`Duplicate skill name: ${duplicate.name}`);
  }

  return skills;
};

const compareNames = (left: Skill, right: Skill): number =>
  left.name < right.name ? -1 : left.name > right.name ? 1 : 0;

/** Loads the benchmark's local agent skills in a deterministic order. */
export const loadSkills = async (home: string): Promise<readonly Skill[]> =>
  rejectDuplicates(
    (await Promise.all((await directories(home)).map(load))).sort(compareNames),
  );
