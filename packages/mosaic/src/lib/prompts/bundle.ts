import type { Node } from '../types/graph.js';
import type { Skill } from 'bundle';

function escapeXml(value: string | number | boolean) {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&apos;');
}

export function system() {
  return [
    'You are the bundle selector for one objective in a goal-oriented plan.',
    '',
    'Select the smallest ordered set of skills whose combined instructions are',
    'sufficient to help complete the current objective.',
    '',
    'A skill may be selected only when:',
    '',
    '1. its body directly applies to the current objective;',
    '2. it adds behavior needed to satisfy at least one doneWhen criterion;',
    '3. that behavior is not already substantially covered by a previously',
    '   selected skill;',
    '4. it does not conflict with previously selected skills.',
    '',
    'Rules:',
    '',
    '- Evaluate skills by their behavioral instructions, not merely by topic,',
    '  name, description, rerank score, or allowed tools.',
    '- Preserve the relative order produced by the reranker.',
    '- Select at most K_max skills.',
    '- Prefer the smallest sufficient bundle.',
    '- Reject skills that are irrelevant, unnecessary, redundant, conflicting,',
    '  or beyond the bundle limit.',
    '- A relevant skill may still be rejected when it adds no distinct behavior.',
    '- The bundle may be empty.',
    '- Do not create, remove, split, or modify plan objectives.',
    '- Return only the requested structured output.',
  ].join('\n');
}

export function user(prompt: string, node: Node, skills: Array<Skill>) {
  return [
    `<prompt>${escapeXml(prompt)}</prompt>`,
    '',
    '<node>',
    `  <id>${escapeXml(node.id)}</id>`,
    `  <goal>${escapeXml(node.goal)}</goal>`,
    '  <doneWhen>',
    ...node.doneWhen.map(
      (criterion) => `    <criterion>${escapeXml(criterion)}</criterion>`,
    ),
    '  </doneWhen>',
    '</node>',
    '',
    '<skills>',
    ...skills.flatMap((skill) => [
      '  <skill>',
      `    <name>${escapeXml(skill.name)}</name>`,
      `    <description>${escapeXml(skill.description)}</description>`,
      `    <body>${escapeXml(skill.body)}</body>`,
      '  </skill>',
    ]),
    '</skills>',
  ].join('\n');
}
