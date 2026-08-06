import type { StateMachineHandler } from 'state-machine';
import type { WorkflowContext, WorkflowState } from '../../types/workflow.js';

import { BundleSchema } from '../../schemas/bundle.js';

const RETRIEVAL_LIMIT = 5;

/**
 * Prepares every node in the selected wave sequentially.
 *
 * This step generates the bundles for each node with a combination of the best skills to achieve the goal.
 */
export const bundle: StateMachineHandler<
  WorkflowContext,
  WorkflowState
> = async ({ graphs }, { input, options }, { transition, fail }) => {
  try {
    const { logger, skills, tools, provider, models } = options;

    logger.info({}, 'generating bundles');

    const graph = graphs.at(-1);

    if (graph === undefined)
      return fail(new Error('Impossible to continue, missing active graph!'));

    // Retrieve only the ready nodes
    const nodes = graph.nodes.filter(({ status }) => status === 'ready');

    /**
     * Here, we are generating a bundle of tools and skills for each ready node,
     * this bundle contains the best skill and tools to achieve the current goal.
     */
    for (const node of nodes) {
      /**
       * We perform the same matching as when generating the graph, but this time,
       * we have the correct context and decomposition for the graph. We expect better
       * results here.
       *
       * @TODO: We should include previous artifacts from other nodes when available
       */
      const matches = await skills.embeddings.search(
        [
          `Original Request:\n${input}`,
          `Current Goal::\n${node.goal}`,
          `Completion Criteria:\n${markdownNumberedList(node.doneWhen)}`,
        ].join('\n\n'),
        RETRIEVAL_LIMIT,
      );

      const candidates = matches.map(({ data }) => data);

      /**
       * The agent reranks the options, so we have the best matches for the current goal (node).
       * This step can limit the ammount of valid options we should return for the next evaluation stage.
       *
       * @TODO: We should include previous artifacts from other nodes when available
       */
      const ranking = await provider.rerank({
        model: models.reranker,
        query: [
          `Original request:\n${input.trim()}`,
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
        ]
          .filter((section): section is string => Boolean(section))
          .join('\n\n'),
        documents: candidates.map((skill) =>
          [
            `Skill name: ${skill.name}`,
            `Description:\n${skill.description.trim()}`,
            `Canonical body:\n${skill.body.trim()}`,
          ].join('\n\n'),
        ),
        topN: RETRIEVAL_LIMIT,
      });

      // Match the available skills with the reranked skills
      const reranked = ranking.map(({ index }) => candidates[index]);

      logger.info(
        {
          skills: reranked.map(
            (skill) => `${skill.name}: ${skill.description}`,
          ),
        },
        'relevant skills',
      );

      const { structured: selected } = await provider.complete({
        model: models.default,
        messages: [
          {
            role: 'system',
            content: [
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
            ].join('\n'),
          },
          {
            role: 'user',
            content: [
              `<prompt>${input}</prompt>`,
              '',
              '<node>',
              `  <id>${node.id}</id>`,
              `  <goal>${node.goal}</goal>`,
              '  <doneWhen>',
              ...node.doneWhen.map(
                (criterion) => `    <criterion>${criterion}</criterion>`,
              ),
              '  </doneWhen>',
              '</node>',
              '',
              '<skills>',
              ...reranked.flatMap((skill) => [
                '  <skill>',
                `    <name>${skill.name}</name>`,
                `    <description>${skill.description}</description>`,
                `    <body>${skill.body}</body>`,
                '  </skill>',
              ]),
              '</skills>',
            ].join('\n'),
          },
        ],
        schema: BundleSchema,
      });

      const bundleSkills = [
        ...skills.required,
        ...selected.skills
          .map((name) => reranked.find((skill) => skill.name === name))
          .filter((skill) => skill !== undefined),
      ];

      const bundleTools = [
        ...tools.required,
        ...bundleSkills
          .map((skill) => skill?.allowedTools ?? [])
          .flat()
          .map((name) => tools.menu.find((tool) => tool.name === name))
          .filter((tool) => tool !== undefined),
      ].map(({ name, description }) => ({
        name,
        description: description ?? '',
      }));

      console.log(bundleTools);
      console.log(bundleSkills);

      node.tools = bundleTools;
      node.skills = bundleSkills;
    }

    return transition('execution', { graphs });
  } catch (error) {
    return fail(error);
  }
};

const markdownNumberedList = (items: Array<string>) =>
  items.map((item, index) => `${index + 1}. ${item}`).join(' \n');
