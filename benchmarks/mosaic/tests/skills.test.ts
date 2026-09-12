import assert from 'node:assert/strict';
import { mkdir, mkdtemp, realpath, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import { loadSkills } from '../src/skills.js';

const createHome = (): Promise<string> =>
  mkdtemp(join(tmpdir(), 'mosaic-skills-'));

const writeSkill = async (
  home: string,
  directory: string,
  source: string,
): Promise<void> => {
  const path = join(home, '.agents', 'skills', directory);

  await mkdir(path, { recursive: true });

  await writeFile(join(path, 'SKILL.md'), source, 'utf8');
};

test('returns no skills when the skill directory is missing or empty', async () => {
  const home = await createHome();

  try {
    assert.deepEqual(await loadSkills(home), []);

    await mkdir(join(home, '.agents', 'skills'), { recursive: true });

    assert.deepEqual(await loadSkills(home), []);
  } finally {
    await rm(home, { recursive: true, force: true });
  }
});

test('ignores sibling directories without a readable skill file', async () => {
  const home = await createHome();

  try {
    await mkdir(join(home, '.agents', 'skills', 'licenses'), {
      recursive: true,
    });

    assert.deepEqual(await loadSkills(home), []);
  } finally {
    await rm(home, { recursive: true, force: true });
  }
});

test('rejects invalid skill frontmatter', async () => {
  const home = await createHome();

  try {
    await writeSkill(home, 'invalid', '---\nname: [\n---\nBody');

    await assert.rejects(loadSkills(home));
  } finally {
    await rm(home, { recursive: true, force: true });
  }
});

test('sorts skills and restricts each skill to the terminal', async () => {
  const home = await createHome();

  try {
    await writeSkill(
      home,
      'second',
      '---\nname: zebra\ndescription: Zebra skill\nallowed-tools: [web]\n---\n# Zebra',
    );

    await writeSkill(
      home,
      'first',
      '---\nname: alpha\ndescription: Alpha skill\nallowed-tools: [write]\n---\n# Alpha',
    );

    const skills = await loadSkills(home);

    assert.deepEqual(
      skills.map((skill) => skill.name),
      ['alpha', 'zebra'],
    );

    assert.deepEqual(
      skills.map((skill) => skill.allowedTools),
      [['terminal'], ['terminal']],
    );

    assert.equal(
      skills[0]?.body,
      `Skill files are in ${await realpath(join(home, '.agents', 'skills', 'first'))}. Resolve scripts/, references/, and other relative paths from this directory.\n\n# Alpha`,
    );
  } finally {
    await rm(home, { recursive: true, force: true });
  }
});

test('rejects duplicate normalized skill names', async () => {
  const home = await createHome();

  try {
    await writeSkill(
      home,
      'one',
      '---\nname: same\ndescription: One\n---\n# One',
    );

    await writeSkill(
      home,
      'two',
      '---\nname: same\ndescription: Two\n---\n# Two',
    );

    await assert.rejects(loadSkills(home), /Duplicate skill name: same/);
  } finally {
    await rm(home, { recursive: true, force: true });
  }
});
