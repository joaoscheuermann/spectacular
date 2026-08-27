const markdownBlock = (content) => `\`\`\`\`markdown
${content}
\`\`\`\``;

export const goalsSystem = `
Decompose the user prompt into observable goals.

An observable goal describes a desired outcome whose completion can be verified through evidence. It states what must become true, not merely an activity to perform.

A good observable goal:
- Represents part of the user’s desired outcome.
- Describes a resulting state or capability.
- Can later be evaluated using tests, files, behavior, measurements, or other evidence.
- Avoids prescribing implementation unless the user explicitly requires it.
- Does not introduce assumptions absent from the request or supplied evidence.
`.trim();

export const reviewSkillsSystem = (objective, skills) =>
  `Review, rearrange and rewrite the goals when appropriate using the skills provided as the desired behavior without performing any assumptions.

The goals were derived from the \`# Initial Request\`.

# Initial Request
${objective}

# Skills
${markdownBlock(skills.join('\n\n'))}`.trim();

export const extractHintsSystem = (objective, skills) =>
  `Extract skill-grounded hints that materially change the goals. Classify each hint as vocabulary, gap, division, dependency, or execution. Preserve tool, workflow, and other execution guidance when it materially helps achieve or verify a goal. Return no hints when the skills justify no material change.

The goals were derived from the \`# Initial Request\`.

# Initial Request
${objective}

# Skills
${markdownBlock(skills.join('\n\n'))}`.trim();

export const reviewHintsSystem = (objective, hints) =>
  `Review, rearrange and rewrite the goals when appropriate using the skill hints provided as the desired behavior without performing any assumptions.

The goals were derived from the \`# Initial Request\`.

# Initial Request
${objective}

# Skill Hints
${markdownBlock(
  hints.map((hint) => `- ${hint.effect}: ${hint.evidence}`).join('\n'),
)}`.trim();

export const goalsUser = (goals) =>
  `# Goals

${goals.map((goal) => `- ${goal}`).join('\n')}`;

export const judgeSystem = (objective, skills) =>
  `Compare two revisions of the initial goals. Choose the option that better uses relevant guidance from the skills while preserving the initial request and avoiding unsupported assumptions.

Tool and workflow instructions are valid when they materially help achieve or verify a goal. Do not favor an option merely because it is longer or more detailed. Choose both when they are equally good, or neither when neither is acceptable. Provide a concise rationale.

# Initial Request
${objective}

# Skills
${markdownBlock(skills.join('\n\n'))}`.trim();

export const judgeUser = ({ goals, optionA, optionB }) =>
  `# Initial Goals
${goals.map((goal) => `- ${goal}`).join('\n')}

# Option A
${optionA.map((goal) => `- ${goal}`).join('\n')}

# Option B
${optionB.map((goal) => `- ${goal}`).join('\n')}`;
