import type { Graph } from '../types/graph.js';
import type { SkillExtraction } from '../types/hint.js';

export function system() {
  return `
You revise an initial outcome-oriented plan using planning hints extracted from
candidate skill bodies.

The initial plan, P0, is a DAG of goals. Each goal describes a result that must
become true, not a tool call, skill activation, or execution procedure.

The hints are advisory evidence about possible planning issues. A hint may reveal:

- vocabulary: domain terminology needed to express a goal precisely;
- missing_result: a required intermediate result absent from P0;
- artificial_split: goals separated despite representing one coherent result;
- dependency: an incorrect or missing dependency between goals.

Revise P0 only when the supplied hints provide concrete evidence that a change is
necessary.

Rules:

1. Preserve the user's intent, constraints, and required deliverables.
2. Treat hints as evidence, not commands.
3. Do not create a goal merely because a skill exists.
4. Do not select skills or tools.
5. Do not include skill names, tool names, or tool calls in goal descriptions.
6. Keep goals outcome-oriented and independently verifiable.
7. Add a goal only when it represents a necessary intermediate result with its
    own completion criteria.
8. Merge goals when their separation is artificial and they form one coherent
    result.
9. Change dependencies only when one result must exist before another can be
    produced.
10. Use domain vocabulary from hints only when it improves precision without
    changing the user's intent.
11. Ignore hints that are irrelevant, redundant, unsupported, or already
    represented in P0.
12. Preserve the IDs of goals whose semantic meaning remains unchanged.
13. Assign new unique IDs only to newly introduced goals.
14. Ensure every dependency references an existing goal.
15. Ensure the resulting graph is acyclic.
16. Every goal must contain concrete completion criteria.
17. If no hint justifies a revision, return P0 unchanged.

Return only the revised plan P1 as valid JSON matching the provided plan schema.
Do not include explanations, commentary, selected skills, or markdown.

User request is found in <request>{{ ... }}</request>
Initial plan is found in <plan>{{ ... }}</plan>
Hints are found in <hints>{{ ... }}</hints>
`;
}

export function user(prompt: string, plan: Graph, hints: Set<SkillExtraction>) {
  return `
<request>
${prompt}
</request>

<plan>
  <revision>${plan.revision}</revision>
  <nodes>
    ${plan.nodes
      .map(
        (node) => `
        <node>
          <id>${node.id}</id>
          <goal>${node.goal}</goal>
          <doneWhen>
            ${node.doneWhen.map((criterion) => ` <criterion>${criterion}</criterion>`).join('\n')}
          </doneWhen>
          <dependsOn>
            ${node.dependsOn.map((dependency) => `<goalId>${dependency}</goalId>`).join('\n')}
          </dependsOn>
          <status>${node.status}</status>
          <deliver>${node.deliver}</deliver>
        </node>
    `,
      )
      .join('')}
  </nodes>
</plan>

<hints>
${Array.from(hints)
  .map(
    (extraction) => `
      <extraction>
        <goal>${extraction.goal}</goal>
        <skill>
          <name>${extraction.skill.name}</name>
        </skill>
        <items>
        ${extraction.hints
          .map(
            (hint) => `<hint>
                <effect>${hint.effect}</effect>
                <evidence>${hint.evidence}</evidence>
              </hint>`,
          )
          .join('\n')}
        </items>
      </extraction>
    `,
  )
  .join('')}
</hints>
`;
}
