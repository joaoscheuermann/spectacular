const skillsBench =
  'https://raw.githubusercontent.com/benchflow-ai/skillsbench/v1.1/tasks';
const skillUrl = (task, skill) =>
  `${skillsBench}/${task}/environment/skills/${skill}/SKILL.md`;

export const cases = [
  {
    name: 'pgvector support',
    objective: `Clone "https://github.com/joaoscheuermann/doric" and add pgvector support to package/victor.`,
    skills: [
      `
# Coding workflow

Before implementation, inspect the affected package, its public contracts, dependants, and existing tests. Define the behavior, write black-box tests that fail for the missing behavior, and then implement without changing those tests.
    `.trim(),
    ],
  },
  {
    name: 'production migration',
    objective: `Deploy the new billing service version to production without interrupting active checkouts. The release includes a database migration.`,
    skills: [
      `
# Safe database release

Treat schema compatibility, application rollout, health verification, and rollback as one release decision. The migration must remain compatible with both application versions during rollout, and rollback must be possible before removing the old schema.
    `.trim(),
    ],
  },
  {
    name: 'budget trip',
    objective: `Plan a five-day trip to Santiago for two people with a total budget of R$7,000, central lodging, and one winery day trip.`,
    skills: [
      `
# Feasible trip plan

Validate transport, lodging, local travel, activities, fees, and a contingency margin together before recommending an itinerary. Do not present choices as final when their combined cost or schedule has not been checked against the user's constraints.
      `.trim(),
    ],
  },
  {
    name: 'SkillsBench: travel planning',
    source:
      'https://github.com/benchflow-ai/skillsbench/tree/v1.1/tasks/travel-planning',
    objective: `Build a 7-day itinerary for two people leaving Minneapolis and visiting three Ohio cities from March 17 to March 23, 2022, within a $5,100 budget. Accommodations must be pet-friendly, meals should include American, Mediterranean, Chinese, and Italian cuisines, and the trip must not use flights. Use real-world data rather than remembered places.`,
    skillUrls: [
      skillUrl('travel-planning', 'search-cities'),
      skillUrl('travel-planning', 'search-accommodations'),
      skillUrl('travel-planning', 'search-restaurants'),
      skillUrl('travel-planning', 'search-attractions'),
      skillUrl('travel-planning', 'search-driving-distance'),
    ],
  },
  {
    name: 'SkillsBench: Spring Boot migration',
    source:
      'https://github.com/benchflow-ai/skillsbench/tree/v1.1/tasks/spring-boot-jakarta-migration',
    objective: `Migrate the user-management microservice in /workspace from Java 8 and Spring Boot 2.7 to Java 21 and Spring Boot 3.2. Preserve CRUD, JWT authentication, role-based access, JPA persistence, validation, and external API behavior. Handle Jakarta namespaces, Hibernate 6, Spring Security 6, replace RestTemplate with RestClient, remove deprecated security APIs, and pass mvn clean compile and mvn test.`,
    skillUrls: [
      skillUrl('spring-boot-jakarta-migration', 'spring-boot-migration'),
      skillUrl('spring-boot-jakarta-migration', 'jakarta-namespace'),
      skillUrl('spring-boot-jakarta-migration', 'hibernate-upgrade'),
      skillUrl('spring-boot-jakarta-migration', 'spring-security-6'),
      skillUrl('spring-boot-jakarta-migration', 'restclient-migration'),
    ],
  },
  {
    name: 'SkillsBench: citation check',
    source:
      'https://github.com/benchflow-ai/skillsbench/tree/v1.1/tasks/citation-check',
    objective: `Inspect /root/test.bib, identify fake or hallucinated academic citations, and write /root/answer.json containing only their cleaned titles in a sorted fake_citations array.`,
    skillUrls: [skillUrl('citation-check', 'citation-management')],
  },
];

export const loadSkills = async ({ skills = [], skillUrls = [] }) => [
  ...skills,
  ...(await Promise.all(
    skillUrls.map(async (url) => {
      const response = await fetch(url);
      if (!response.ok)
        throw new Error(`Could not load external skill: ${url}`);
      return response.text();
    }),
  )),
];

export const runCases = cases.map(({ name, objective, source }) => ({
  name,
  objective,
  ...(source === undefined ? {} : { source }),
}));
