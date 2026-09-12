export const skillsbenchRevision = 'b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af';

const sources = [
  ['data-to-d3', 'd3-visualization'],
  ['earthquake-phase-association', 'gamma-phase-associator'],
  ['earthquake-phase-association', 'obspy-data-api'],
  ['earthquake-phase-association', 'seisbench-model-api'],
  ['earthquake-phase-association', 'seismic-picker-selection'],
  ['edit-pdf', 'pdf-editing'],
  ['edit-pdf', 'text-parser'],
  ['jax-computing-basics', 'jax-skills'],
  ['organize-messy-files', 'docx'],
  ['organize-messy-files', 'file-organizer'],
  ['organize-messy-files', 'pdf'],
  ['organize-messy-files', 'planning-with-files'],
  ['organize-messy-files', 'pptx'],
  ['sec-financial-report', '13f-analyzer'],
  ['sec-financial-report', 'fuzzy-name-search'],
  ['spring-boot-jakarta-migration', 'hibernate-upgrade'],
  ['spring-boot-jakarta-migration', 'jakarta-namespace'],
  ['spring-boot-jakarta-migration', 'restclient-migration'],
  ['spring-boot-jakarta-migration', 'spring-boot-migration'],
  ['spring-boot-jakarta-migration', 'spring-security-6'],
  ['travel-planning', 'search-accommodations'],
  ['travel-planning', 'search-attractions'],
  ['travel-planning', 'search-cities'],
  ['travel-planning', 'search-driving-distance'],
  ['travel-planning', 'search-flights'],
  ['travel-planning', 'search-restaurants'],
  ['xlsx-recover-data', 'data-reconciliation'],
  ['xlsx-recover-data', 'xlsx'],
  ['citation-check', 'citation-management'],
];
const urlOf = (task, directory) =>
  `https://raw.githubusercontent.com/benchflow-ai/skillsbench/${skillsbenchRevision}/tasks/${task}/environment/skills/${directory}/SKILL.md`;

const field = (metadata, name) => {
  const prefix = `${name}:`;

  const line = metadata
    .split(/\r?\n/u)
    .find((entry) => entry.startsWith(prefix));

  if (line === undefined) {throw new Error(`SkillsBench skill has no ${name}.`);}

  const value = line.slice(prefix.length).trim();

  if (value.startsWith('"')) {return JSON.parse(value);}

  if (value.startsWith("'") && value.endsWith("'")) {return value.slice(1, -1);}

  if (value === '') {throw new Error(`SkillsBench skill has an empty ${name}.`);}

  return value;
};

const parseSkill = (source, url) => {
  const match = /^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)([\s\S]*)$/u.exec(
    source,
  );

  if (match === null) {throw new Error(`Invalid SkillsBench skill: ${url}`);}

  return {
    name: field(match[1], 'name'),
    description: field(match[1], 'description'),
    body: match[2].trim(),
    source: url,
  };
};

export const loadSkillsbenchCatalog = () =>
  Promise.all(
    sources.map(async ([task, directory]) => {
      const url = urlOf(task, directory);
      const response = await fetch(url);

      if (!response.ok)
        {throw new Error(`Could not load SkillsBench skill: ${url}`);}

      return parseSkill(await response.text(), url);
    }),
  );
