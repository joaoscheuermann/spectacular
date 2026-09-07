import { createHash } from 'node:crypto';
import { readFile, readdir } from 'node:fs/promises';
import { basename, dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { caseSchema, fixtureSchema } from './schemas.mjs';

export const sourceRunId = '89ba6c1e-470c-43ea-809b-a34a90f59540';
export const sourceResultsSha256 =
  'f904b241cc05264c281e017ebfb16cbe52af934c4d4c7bc6b62f71088f5b68db';
export const frozenFixtureSha256 =
  '909bfc7d5153a08bfbe516604d68b3255905bbddb267e4334aed7ec94a7d4908';

const directory = dirname(fileURLToPath(import.meta.url));
const casesDirectory = join(directory, 'cases');
const skillsDirectory = join(casesDirectory, 'skills');
const fixturePath = join(directory, 'fixtures', 'p0.json');

const loadFixture = async () => {
  const data = await readFile(fixturePath);
  const sha256 = createHash('sha256').update(data).digest('hex');
  if (sha256 !== frozenFixtureSha256) {
    throw new Error(`Unexpected frozen fixture hash: ${sha256}`);
  }
  return fixtureSchema.parse(JSON.parse(data.toString('utf8')));
};

const loadCases = async () => {
  const entries = (await readdir(casesDirectory, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && extname(entry.name) === '.json')
    .sort((left, right) => left.name.localeCompare(right.name));

  return Promise.all(
    entries.map(async ({ name }) =>
      caseSchema.parse(
        JSON.parse(await readFile(join(casesDirectory, name), 'utf8')),
      ),
    ),
  );
};

const loadCatalog = async () => {
  const entries = (await readdir(skillsDirectory, { withFileTypes: true }))
    .filter((entry) => entry.isFile() && extname(entry.name) === '.md')
    .sort((left, right) => left.name.localeCompare(right.name));

  return Promise.all(
    entries.map(async ({ name }) => {
      const body = (await readFile(join(skillsDirectory, name), 'utf8')).trim();
      if (body.length === 0) throw new Error(`Skill is empty: ${name}`);
      return Object.freeze({ name: basename(name, '.md'), body });
    }),
  );
};

const uniqueByName = (values, label) => {
  const names = new Set();
  for (const { name } of values) {
    if (names.has(name)) throw new Error(`Duplicate ${label}: ${name}`);
    names.add(name);
  }
  return names;
};

const validateFixtureSource = (fixture) => {
  if (fixture.source.runId !== sourceRunId) {
    throw new Error(`Unexpected fixture run ID: ${fixture.source.runId}`);
  }
  if (fixture.source.resultsSha256 !== sourceResultsSha256) {
    throw new Error(
      `Unexpected source results hash: ${fixture.source.resultsSha256}`,
    );
  }
};

const validateCaseSets = (cases, fixtureCases) => {
  const localNames = uniqueByName(cases, 'local case');
  const fixtureNames = uniqueByName(fixtureCases, 'fixture case');
  const missing = [...localNames].filter((name) => !fixtureNames.has(name));
  const unknown = [...fixtureNames].filter((name) => !localNames.has(name));

  if (missing.length > 0) {
    throw new Error(`Fixture is missing cases: ${missing.join(', ')}`);
  }
  if (unknown.length > 0) {
    throw new Error(`Fixture has unknown cases: ${unknown.join(', ')}`);
  }
};

const validateCatalogPartitions = (cases, catalog) => {
  const catalogNames = new Set(catalog.map(({ name }) => name));
  for (const current of cases) {
    const classified = new Set([
      ...current.skills.expected,
      ...current.skills.useful,
      ...Object.keys(current.skills.noise),
    ]);
    const unknown = [...classified].filter((name) => !catalogNames.has(name));
    const missing = [...catalogNames].filter((name) => !classified.has(name));
    if (unknown.length > 0) {
      throw new Error(
        `Unknown classified skills for ${current.name}: ${unknown.join(', ')}`,
      );
    }
    if (missing.length > 0) {
      throw new Error(
        `Unclassified skills for ${current.name}: ${missing.join(', ')}`,
      );
    }
  }
};

const resolveCases = (cases, fixture, catalog) => {
  const frozenByName = new Map(
    fixture.cases.map((current) => [current.name, current]),
  );
  const catalogByName = new Map(catalog.map((skill) => [skill.name, skill]));

  return cases.map((current) => {
    const frozen = frozenByName.get(current.name);
    if (frozen === undefined) {
      throw new Error(`Fixture is missing case: ${current.name}`);
    }
    if (frozen.objective !== current.objective) {
      throw new Error(`Objective mismatch for case: ${current.name}`);
    }

    const goldNames = [...current.skills.expected, ...current.skills.useful];
    if (new Set(goldNames).size !== goldNames.length) {
      throw new Error(`Duplicate gold skill for case: ${current.name}`);
    }

    const goldSkills = goldNames.map((name) => {
      const skill = catalogByName.get(name);
      if (skill === undefined) {
        throw new Error(`Unknown gold skill for ${current.name}: ${name}`);
      }
      return skill;
    });

    return Object.freeze({
      name: current.name,
      objective: current.objective,
      p0: Object.freeze([...frozen.p0]),
      goldSkills: Object.freeze(goldSkills),
    });
  });
};

export const loadInputs = async () => {
  const [cases, catalog, fixture] = await Promise.all([
    loadCases(),
    loadCatalog(),
    loadFixture(),
  ]);

  if (cases.length !== 30) {
    throw new Error(`Expected 30 local cases; found ${cases.length}.`);
  }
  if (catalog.length !== 37) {
    throw new Error(`Expected 37 catalog skills; found ${catalog.length}.`);
  }
  uniqueByName(catalog, 'catalog skill');
  validateFixtureSource(fixture);
  validateCaseSets(cases, fixture.cases);
  validateCatalogPartitions(cases, catalog);

  return Object.freeze({
    fixture,
    catalog: Object.freeze(catalog),
    cases: Object.freeze(resolveCases(cases, fixture, catalog)),
  });
};
