import { LlmProvider } from 'llms';
import { Node } from '../../types/graph.js';
import { Skill } from '../../types/skill.js';
import { Logger } from 'pino';

interface RerankContext {
  provider: LlmProvider;
  logger: Logger;
}

/**
 * Formats a skill as one rerank document.
 *
 * allowedTools is intentionally omitted from the relevance document.
 * A skill should rank highly because its instructions apply to the goal,
 * not merely because it exposes a potentially useful tool.
 */
function document(skill: Skill): string {
  return [
    `Skill name: ${skill.name}`,
    `Description:\n${skill.description.trim()}`,
    `Canonical body:\n${skill.body.trim()}`,
  ].join('\n\n');
}

function query(prompt: string, node: Node) {
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
  ];

  return sections
    .filter((section): section is string => Boolean(section))
    .join('\n\n');
}

export default async function rerank(
  prompt: string,
  node: Node,
  skills: Array<Skill>,
  topN: number,
  { provider }: RerankContext,
) {
  const result = await provider.rerank({
    model: 'voyageai/rerank-2.5-lite',
    query: query(prompt, node),
    documents: skills.map(document),
    topN,
  });

  console.log(JSON.stringify(result));
}
