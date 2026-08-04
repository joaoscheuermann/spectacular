import type { Skill } from 'bundle';

import * as bundlePrompt from '../../prompts/bundle.js';
import { markdownNumberedList } from '../../prompts/list.js';
import { rerankQuery } from '../../prompts/rerank.js';
import { BundleSchema } from '../../schemas/bundle.js';
import type { Node } from '../../types/graph.js';
import type { WorkflowContext } from '../../types/workflow.js';
import { candidateSkills, composeMenus } from './menus.js';

const RETRIEVAL_LIMIT = 10;

/** Retrieves, reranks, and composes the skill and tool menus for one node. */
export const prepareNode = async (
  node: Node,
  { input, options }: WorkflowContext,
): Promise<void> => {
  const { provider, models, skills, tools } = options;
  const matches = await skills.embeddings.search(
    [
      `Original Request:\n${input}`,
      `Current Goal::\n${node.goal}`,
      `Completion Criteria:\n${markdownNumberedList(node.doneWhen)}`,
    ].join('\n\n'),
    RETRIEVAL_LIMIT,
  );
  const candidates = matches.map(({ data }) => data);
  const ranking = await provider.rerank({
    model: models.reranker,
    query: rerankQuery(input, node),
    documents: candidates.map(skillDocument),
    topN: RETRIEVAL_LIMIT,
  });
  const reranked = ranking.map(({ index }) => candidates[index]);
  const available = candidateSkills(skills.required, reranked);
  const { structured: selected } = await provider.complete({
    model: models.default,
    messages: [
      { role: 'system', content: bundlePrompt.system() },
      { role: 'user', content: bundlePrompt.user(input, node, available) },
    ],
    schema: BundleSchema,
  });
  const menus = composeMenus({
    requiredSkills: skills.required,
    selected: selected.skills,
    skillMenu: skills.menu,
    requiredTools: tools.required,
    toolMenu: tools.menu,
  });

  console.log(JSON.stringify(selected));
  console.log(JSON.stringify(menus.skills));
  console.log(JSON.stringify(menus.tools));
};

const skillDocument = (skill: Skill): string =>
  [
    `Skill name: ${skill.name}`,
    `Description:\n${skill.description.trim()}`,
    `Canonical body:\n${skill.body.trim()}`,
  ].join('\n\n');
