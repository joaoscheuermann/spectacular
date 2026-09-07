const numbered = (goals) =>
  goals.map((goal, index) => `${index + 1}. ${goal}`).join('\n');

const skillBlock = ({ body }, index) => `## Guidance ${index + 1}

\`\`\`\`markdown
${body}
\`\`\`\``;

const skillBlocks = (skills) => skills.map(skillBlock).join('\n\n');

export const directPlanSystem = `Create a complete plan of observable, verifiable goals.

Rules:

1. The objective is the only authority for scope and deliverables.
2. Preserve every requirement supported by the objective.
3. Selected skills are advisory operational guidance.
4. Skills may add applicable methods and verification criteria, but never scope or deliverables that the objective does not request.
5. Return a complete, standalone plan.
6. Every goal must describe an observable, verifiable result rather than an activity.
7. Do not mention skill names and do not create goals whose outcome is merely invoking a tool.

Check the final plan against the objective so that no requirement is lost and no unsupported requirement is introduced.`;

const finalPlanPrefix = (objective, skills) => `# Objective

${objective}

# Selected Skills

${skillBlocks(skills)}`;

export const withP0User = (objective, skills, p0) => `${finalPlanPrefix(
  objective,
  skills,
)}

# P0 Draft

${numbered(p0)}`;

export const comparisonSystem = `Compare two candidate plans for the supplied objective.

Apply this rubric in order:

1. Fidelity to the objective and absence of invented scope.
2. Complete coverage of the objective.
3. Correct use of only the materially applicable operational guidance.
4. Observable and verifiable goals.

Do not prefer a plan because it is longer or has more detail. Choose \`a\` or \`b\` only when one plan is materially better. Choose \`both\` when they are equally adequate and \`neither\` when both are materially inadequate.`;

export const comparisonUser = (
  objective,
  skills,
  optionA,
  optionB,
) => `# Objective

${objective}

# Selected Skills

${skillBlocks(skills)}

# Option A

${numbered(optionA)}

# Option B

${numbered(optionB)}`;
