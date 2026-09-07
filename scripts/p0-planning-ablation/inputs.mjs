import { createHash } from 'node:crypto';
import { readFile, readdir } from 'node:fs/promises';
import { basename, dirname, extname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { caseSchema, fixtureSchema } from './schemas.mjs';

export const defaultFixtureName = 'p0.json';
export const controlFixtureName = 'p0-skill-aware-qwen.json';

const frozenFixtures = Object.freeze({
  'p0.json': Object.freeze({
    sha256: '909bfc7d5153a08bfbe516604d68b3255905bbddb267e4334aed7ec94a7d4908',
    sourceRunId: '89ba6c1e-470c-43ea-809b-a34a90f59540',
    sourceResultsSha256:
      'f904b241cc05264c281e017ebfb16cbe52af934c4d4c7bc6b62f71088f5b68db',
  }),
  'p0-skill-aware-qwen.json': Object.freeze({
    sha256: '4d4af3e9de0f002ef7983f5ab31b0c7826a0d80191482e14944abaf2dc6e5323',
    sourceRunId: 'aaae19f2-6f98-4539-8eda-e72bdf8b4f57',
    sourceResultsSha256:
      'd4de560b0a21f4a8dcb185de1751a970e15e9fbe4ba5a72b4e42470047dc6795',
  }),
  'p0-skill-aware-gemini.json': Object.freeze({
    sha256: 'b0bca76f4429b7ec9f2af8fd931f7c19a726c8b74639b92b7859b61a594aeac0',
    sourceRunId: 'd95fc9dd-42b8-4bfc-8358-7dcfae141198',
    sourceResultsSha256:
      'c920e22d338f0d6d2b74f262a883585fb9a9c0e1577e61bd3c7d75c0fe24847e',
  }),
});

const directory = dirname(fileURLToPath(import.meta.url));
const casesDirectory = join(directory, 'cases');
const skillsDirectory = join(casesDirectory, 'skills');

const loadFixture = async (fixtureName) => {
  const expected = frozenFixtures[fixtureName];
  if (expected === undefined) {
    throw new Error(`Unknown frozen fixture: ${fixtureName}`);
  }
  const fixturePath = join(directory, 'fixtures', fixtureName);
  const data = await readFile(fixturePath);
  const sha256 = createHash('sha256').update(data).digest('hex');
  if (sha256 !== expected.sha256) {
    throw new Error(`Unexpected frozen fixture hash: ${sha256}`);
  }
  const fixture = fixtureSchema.parse(JSON.parse(data.toString('utf8')));
  validateFixtureSource(fixture, expected);
  return fixture;
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

const validateFixtureSource = (fixture, expected) => {
  if (fixture.source.runId !== expected.sourceRunId) {
    throw new Error(`Unexpected fixture run ID: ${fixture.source.runId}`);
  }
  if (fixture.source.resultsSha256 !== expected.sourceResultsSha256) {
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

const resolveCases = (cases, fixture, catalog, controlFixture) => {
  const frozenByName = new Map(
    fixture.cases.map((current) => [current.name, current]),
  );
  const controlByName =
    controlFixture === undefined
      ? undefined
      : new Map(controlFixture.cases.map((current) => [current.name, current]));
  const catalogByName = new Map(catalog.map((skill) => [skill.name, skill]));

  return cases.map((current) => {
    const frozen = frozenByName.get(current.name);
    if (frozen === undefined) {
      throw new Error(`Fixture is missing case: ${current.name}`);
    }
    if (frozen.objective !== current.objective) {
      throw new Error(`Objective mismatch for case: ${current.name}`);
    }
    const control = controlByName?.get(current.name);
    if (controlByName !== undefined && control === undefined) {
      throw new Error(`Control fixture is missing case: ${current.name}`);
    }
    if (control !== undefined && control.objective !== current.objective) {
      throw new Error(`Control objective mismatch for case: ${current.name}`);
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
      ...(control === undefined
        ? {}
        : { controlPlan: Object.freeze([...control.p0]) }),
      goldSkills: Object.freeze(goldSkills),
    });
  });
};

export const loadInputs = async ({
  fixtureName = defaultFixtureName,
  withControl = false,
} = {}) => {
  const [cases, catalog, fixture, controlFixture] = await Promise.all([
    loadCases(),
    loadCatalog(),
    loadFixture(fixtureName),
    withControl ? loadFixture(controlFixtureName) : undefined,
  ]);

  if (cases.length !== 30) {
    throw new Error(`Expected 30 local cases; found ${cases.length}.`);
  }
  if (catalog.length !== 37) {
    throw new Error(`Expected 37 catalog skills; found ${catalog.length}.`);
  }
  uniqueByName(catalog, 'catalog skill');
  validateCaseSets(cases, fixture.cases);
  if (controlFixture !== undefined) {
    validateCaseSets(cases, controlFixture.cases);
  }
  validateCatalogPartitions(cases, catalog);

  return Object.freeze({
    fixtureName,
    fixture,
    ...(controlFixture === undefined
      ? {}
      : { controlFixtureName, controlFixture }),
    catalog: Object.freeze(catalog),
    cases: Object.freeze(resolveCases(cases, fixture, catalog, controlFixture)),
  });
};
