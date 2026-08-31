const planningSystem = `Decompose the objective into observable goals.

Each goal must describe a verifiable outcome, preserve the objective, avoid any unsupported assumptions, and state what must become true rather than an activity to perform.`;

const goalsList = (goals) =>
  goals.map((goal, index) => `${index + 1}. ${goal}`).join('\n');

const skillBlock = ({ name, body }) => `## ${name}

\`\`\`\`markdown
${body}
\`\`\`\``;

export const p0System = planningSystem;

export const p0User = (objective) => `# Objective
${objective}`;

export const retrievalQuery = (objective, goal) => `
# Objective
${objective}

# Goal
${goal}
`;

export const gateSystem = `Decide whether one retrieved skill materially helps execute or verify the current goal within the objective.

- Use \`keep\` when the skill adds an applicable procedure, constraint, check, tool capability, or data-access method.
- Use \`drop\` when it is irrelevant, conflicting, or adds no useful operational guidance.
- Do not drop useful guidance merely because the goal already names the desired outcome.
- Return a concise reason grounded in this objective, goal, and skill.`;

export const gateUser = (objective, goal, skill) => `# Objective
${objective}

# Goal
${goal}

# Skill
${skillBlock(skill)}`;

export const p1System = `${planningSystem}

Revise P0 only when the supplied skills add useful guidance. Express applicable guidance as observable outcomes or verification criteria. Do not mention skill names.`;

export const p1User = (objective, p0, skills) => `# Objective
${objective}

# P0 Goals
${goalsList(p0)}

# Selected Skills
${skills.map(skillBlock).join('\n\n')}`;
