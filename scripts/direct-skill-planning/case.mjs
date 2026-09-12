import { loadSkillsbenchCatalog } from './skillsbench.mjs';

const localCatalog = [
  {
    name: 'coding-workflow',
    description:
      'Plans behavior-preserving software changes from repository evidence.',
    body: `# Coding workflow

Inspect the affected package, public contracts, dependants, and existing tests before implementation. Define the missing behavior and its observable proof before changing production code. Preserve unrelated behavior and validate through public boundaries.`,
  },
  {
    name: 'vector-database-integration',
    description:
      'Adds vector-database capabilities without weakening existing APIs.',
    body: `# Vector database integration

Treat extension setup, vector type and distance semantics, index lifecycle, and query behavior as one compatible capability. Preserve existing lexical and in-memory behavior, expose the new behavior through the package public API, and verify both supported and unavailable-database paths.`,
  },
  {
    name: 'safe-database-release',
    description:
      'Coordinates compatible database migrations and application rollouts.',
    body: `# Safe database release

Treat schema compatibility, application rollout, health verification, and rollback as one release decision. The migration must remain compatible with both application versions during rollout, and rollback must be possible before removing the old schema.`,
  },
  {
    name: 'feasible-trip-plan',
    description:
      'Validates an itinerary against its complete budget and schedule.',
    body: `# Feasible trip plan

Validate transport, lodging, local travel, activities, fees, and a contingency margin together before recommending an itinerary. Do not present choices as final when their combined cost or schedule has not been checked against the user's constraints.`,
  },
  {
    name: 'real-world-travel-research',
    description:
      'Grounds travel recommendations in current, attributable sources.',
    body: `# Real-world travel research

Use current source evidence for routes, accommodation policies, prices, opening times, and venue details. Keep source dates and assumptions visible, and distinguish verified availability from a plausible recommendation that still requires confirmation.`,
  },
  {
    name: 'road-trip-routing',
    description:
      'Builds feasible multi-city driving routes without hidden flight legs.',
    body: `# Road-trip routing

Validate city order, driving time, rest periods, local transfers, and arrival constraints as one route. Reject an itinerary whose travel legs make its lodging, meal, or activity schedule infeasible.`,
  },
  {
    name: 'citation-verification',
    description:
      'Checks whether academic citations correspond to real publications.',
    body: `# Citation verification

Verify citations against authoritative bibliographic sources using title, authors, venue, year, and persistent identifiers. Classify a citation as fake only when the combined evidence cannot identify a real matching work; preserve uncertainty instead of inventing a match.`,
  },
  {
    name: 'strict-json-delivery',
    description:
      'Produces a machine-consumable JSON artifact with no extra content.',
    body: `# Strict JSON delivery

Validate the final artifact against its requested keys, value types, ordering rules, and output path. Do not include prose, Markdown, or undeclared fields in a file that the user requires to contain only JSON.`,
  },
];

export const cases = [
  {
    name: 'pgvector support',
    objective:
      'Clone "https://github.com/joaoscheuermann/doric" and add pgvector support to package/victor.',
    expectedSkills: ['coding-workflow', 'vector-database-integration'],
  },
  {
    name: 'production migration',
    objective:
      'Deploy the new billing service version to production without interrupting active checkouts. The release includes a database migration.',
    expectedSkills: ['safe-database-release'],
  },
  {
    name: 'budget trip',
    objective:
      'Plan a five-day trip to Santiago for two people with a total budget of R$7,000, central lodging, and one winery day trip.',
    expectedSkills: ['feasible-trip-plan', 'real-world-travel-research'],
  },
  {
    name: 'multi-city road trip',
    objective:
      'Build a seven-day itinerary for two people leaving Minneapolis and visiting three Ohio cities within a $5,100 budget. Accommodations must be pet-friendly, meals must cover four requested cuisines, and the trip must not use flights. Use real-world data rather than remembered places.',
    expectedSkills: [
      'feasible-trip-plan',
      'real-world-travel-research',
      'road-trip-routing',
      'search-accommodations',
      'search-attractions',
      'search-cities',
      'search-driving-distance',
      'search-restaurants',
    ],
  },
  {
    name: 'Spring Boot migration',
    objective:
      'Migrate a user-management microservice from Java 8 and Spring Boot 2.7 to Java 21 and Spring Boot 3.2. Preserve CRUD, JWT authentication, role-based access, JPA persistence, validation, and external API behavior. Handle Jakarta namespaces, Hibernate 6, Spring Security 6, replace RestTemplate with RestClient, and pass mvn clean compile and mvn test.',
    expectedSkills: [
      'coding-workflow',
      'hibernate-upgrade',
      'jakarta-namespace',
      'restclient-migration',
      'spring-boot-migration',
      'spring-security-6',
    ],
  },
  {
    name: 'citation check',
    objective:
      'Inspect /root/test.bib, identify fake or hallucinated academic citations, and write /root/answer.json containing only their cleaned titles in a sorted fake_citations array.',
    expectedSkills: [
      'citation-management',
      'citation-verification',
      'strict-json-delivery',
    ],
  },
];

export const loadCatalog = async () => {
  const catalog = [...localCatalog, ...(await loadSkillsbenchCatalog())];
  const names = new Set(catalog.map(({ name }) => name));

  if (names.size !== catalog.length) {throw new Error('Duplicate skill names.');}

  return catalog;
};

export const selectCases = (name) => {
  if (name === undefined) {return cases;}

  const selected = cases.find((current) => current.name === name);

  if (selected !== undefined) {return [selected];}

  throw new Error(`Unknown case: ${name}`);
};

export const tasksFor = (selected, rounds) =>
  selected.flatMap((current) =>
    Array.from({ length: rounds }, (_, index) => ({
      current,
      round: index + 1,
    })),
  );

export const expectedFor = (catalog, current) => {
  const byName = new Map(catalog.map((skill) => [skill.name, skill]));

  return current.expectedSkills.map((name) => {
    const skill = byName.get(name);

    if (skill === undefined) {throw new Error(`Unknown expected skill: ${name}`);}

    return skill;
  });
};
