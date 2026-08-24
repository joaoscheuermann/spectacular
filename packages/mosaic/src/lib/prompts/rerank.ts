import type { Node } from '../types/graph.js';

export function rerankQuery(prompt: string, node: Node) {
  const sections = [
    `Original request:\n${prompt.trim()}`,

    `Current objective:\n${node.goal.trim()}`,

    [
      'Completion criteria:',
      ...node.doneWhen.map((criterion) => `- ${criterion.trim()}`),
    ].join('\n'),

    // node.stateSummary?.trim()
    //   ? `Relevant prior results:\n${node.stateSummary.trim()}`
    //   : undefined,

    [
      'Ranking instruction:',
      'Rank each skill according to how directly and specifically its',
      'instructions help complete the current objective and satisfy its',
      'completion criteria. Prefer applicable procedural guidance over',
      'generic topical similarity.',
    ].join(' '),
    ].filter((section): section is string => Boolean(section))
    .join('\n\n');

  return sections
    ;
}
