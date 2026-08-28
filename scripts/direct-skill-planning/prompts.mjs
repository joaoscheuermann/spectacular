import {
  goalsSystem,
  reviewSkillsSystem,
} from '../full-skill-vs-hints/prompts.mjs';

const markdownBlock = (content) => `\`\`\`\`markdown
${content}
\`\`\`\``;

const skillRules = `# Skill Use Rules

- Apply only guidance that materially improves the goals for the initial request.
- Ignore irrelevant or conflicting skill content.
- Express useful guidance as observable outcomes or verification criteria.
- Do not mention skill names or write goals as instructions to invoke a skill.
- Preserve the request and avoid unsupported assumptions.
`;

export const directSystem = (skills) =>
  `${goalsSystem}

${skillRules}

# Retrieved Skills
${markdownBlock(skills.join('\n\n'))}`.trim();

export const revisionSystem = (objective, skills) =>
  `${goalsSystem}

${reviewSkillsSystem(objective, skills)}

${skillRules}`.trim();

export const requestRankingQuery = (objective) => `# Original Request
${objective}

# Ranking Instruction
Rank each skill by how directly and specifically its complete instructions would improve decomposition of this request into observable goals. Prefer applicable behavioral guidance over topical similarity.`;

export const goalRankingQuery = (objective, goal) => `# Original Request
${objective}

# Current Goal
${goal}

# Ranking Instruction
Rank each skill by how directly and specifically its complete instructions would materially improve this goal. Prefer applicable behavioral guidance over topical similarity.`;

export const skillDocument = ({ name, description, body }) => `# ${name}

${description}

${body}`;

export const comparisonSystem = (objective, skills) =>
  `Compare A and B using only the initial request and supplied skills. Prefer the option that most faithfully and completely expresses applicable guidance as observable, verifiable goals. Treat unsupported requirements and procedural steps as defects. Do not prefer an option because it is longer or more detailed. Prioritize fidelity, then coverage, then observability.

Set \`choice\` to \`a\` if A is better, \`b\` if B is better, \`both\` if they are equally good under these criteria, or \`neither\` if neither is acceptable. Provide a concise \`rationale\`.

# Initial Request
${objective}

# Skills
${markdownBlock(skills.join('\n\n'))}`.trim();

export const comparisonUser = ({ optionA, optionB }) => `# Option A
${optionA.map((goal, index) => `${index + 1}. ${goal}`).join('\n')}

# Option B
${optionB.map((goal, index) => `${index + 1}. ${goal}`).join('\n')}`;
