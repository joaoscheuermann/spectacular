import { LlmProvider } from 'llms';
import { Logger } from 'pino';
import { VectorDatabase } from 'victor';
import { z } from 'zod';
import { NodeSchema } from '../../schemas/graph/index.js';
import { SkillHintExtractionSchema } from '../../schemas/hint/index.js';
import { SkillExtraction } from '../../types/hint.js';
import { Skill } from '../../types/skill.js';
import { markdownNumberedList } from '../utils/index.js';
import { Graph } from '../../types/graph.js';
import { GoalSkills } from '../candidates/index.js';

type Node = z.output<typeof NodeSchema>;

interface HintsContext {
  logger: Logger;
  provider: LlmProvider;
  vectors: VectorDatabase<Skill>;
}

const system = () => `
Evaluate whether the candidate skill reveals information that should change the
initial plan for the given goal.

A hint is valid only when the skill body provides evidence of one of these effects:

- vocabulary: useful domain terminology absent from the request or goal;
- gap: an intermediate result required by the task is absent;
- division: objectives are separated even though they represent one coherent result;
- dependency: an objective requires a result that must be produced or confirmed earlier.

Do not recommend selecting or executing the skill.
Do not create a goal merely because the skill exists.
Do not describe tools or execution steps unless they reveal a planning dependency.
Return no hint when the skill does not justify a plan change.

The existing goals are provided on <goals>{{ ... }}</goals>
The current goal is provided on <goal>{{ ... }}</goal>
The current skill is provided on <skill>{{ ... }}</skill>
`;

const user = (nodes: Array<Node>, node: Node, skill: Skill) => `
<goals>
${nodes
  .map(
    (node) => `
  <goal>
    <id>${node.id}</id>
    <goal>${node.goal}</goal>
    <doneWhen>
    ${markdownNumberedList(node.doneWhen)}
    </doneWhen>
    <dependsOn>
    ${markdownNumberedList(node.dependsOn)}
    </dependsOn>
  </goal>
`,
  )
  .join('\n')}
</goals>

<goal>
  <id>${node.id}</id>
  <goal>${node.goal}</goal>
  <doneWhen>
  ${markdownNumberedList(node.doneWhen)}
  </doneWhen>
</goal>

<skill>
  <name>${skill.name}</name>
  <body>
  ${skill.body}
  </body>
</skill>
`;

export default async function hints(
  model: string,
  graph: Graph,
  skills: Array<GoalSkills>,
  { provider, logger }: HintsContext,
): Promise<Set<SkillExtraction>> {
  const requests = skills.flatMap((bundle) => {
    const node = graph.nodes.find((node) => node.id === bundle.goal);

    if (!node) throw new Error('Goal not found!');

    logger.info({
      msg: 'extracting hints for goal',
      goal: node.goal,
    });

    return bundle.skills.map(async (skill) => {
      logger.info({
        msg: 'extracting hints from skill',
        goal: node.goal,
        skill: skill.name,
        description: skill.description,
      });

      const { structured } = await provider.complete({
        messages: [
          {
            role: 'system',
            content: system(),
          },
          {
            role: 'user',
            content: user(graph.nodes, node, skill),
          },
        ],
        model,
        schema: SkillHintExtractionSchema,
      });

      if (!structured.hints.length) return;

      return {
        goal: node.goal,
        skill,
        hints: structured.hints,
      };
    });
  });

  const extractions = (await Promise.all(requests)).filter(
    (extraction): extraction is SkillExtraction => extraction !== undefined,
  );

  return new Set(extractions);
}
