import type { Skill } from 'bundle';

import type { Node } from '../types/graph.js';
import { markdownNumberedList } from './list.js';

export function system() {
  return `
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
}

export function user(nodes: Array<Node>, node: Node, skill: Skill) {
  return `
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
}
