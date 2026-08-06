import { createAgent } from 'agent';
import type { Skill } from 'bundle';
import { createMessageStorage } from 'messages';
import type { StateMachineHandler } from 'state-machine';
import { createToolStorage, type Tool } from 'tool';

import type { Node } from '../../types/graph.js';
import type { WorkflowContext, WorkflowState } from '../../types/workflow.js';

/** Executes the active wave with node-specific skills and executable tools. */
export const execution: StateMachineHandler<
  WorkflowContext,
  WorkflowState
> = async ({ graphs }, { options }, { transition, finish, fail }) => {
  const graph = graphs.at(-1);

  if (graph === undefined) return finish();

  const nodes = graph.nodes.filter(({ status }) => status === 'ready');

  if (nodes.length === 0)
    return fail(new Error('Impossible to continue, missing ready nodes!'));

  try {
    await Promise.all(nodes.map((node) => execute(node, options)));

    // return transition('schedule', { graphs });
    return finish();
  } catch (error) {
    return fail(error);
  }
};

const execute = async (node: Node, options: WorkflowContext['options']) => {
  const tools = executors(node, options.tools.required, options.tools.menu);

  const agent = createAgent({
    provider: options.provider,
    tools: createToolStorage(tools),
    messages: createMessageStorage(),
    system: system(node.skills, tools),
    model: options.models.default,
  });

  node.status = 'running';

  options.logger.info(
    {
      nodeId: node.id,
      skills: node.skills.map(({ name }) => name),
      tools: tools.map(({ name }) => name),
    },
    'executing node',
  );

  try {
    const result = await agent.complete(node.goal);

    console.log(result);

    node.status = 'completed';
    options.logger.info({ nodeId: node.id }, 'node execution completed');
  } catch (error) {
    node.status = 'failed';
    options.logger.info({ nodeId: node.id }, 'node execution failed');
    throw error;
  }
};

const executors = (
  node: Node,
  required: readonly Tool[],
  menu: readonly Tool[],
): Tool[] => {
  const catalog = new Map(
    [...required, ...menu].map((tool) => [tool.name, tool]),
  );
  const names = new Set<string>();

  return node.tools.flatMap(({ name }) => {
    if (names.has(name)) return [];

    names.add(name);
    const tool = catalog.get(name);

    if (tool === undefined) {
      throw new Error(`Node ${node.id} references an unavailable tool: ${name}`);
    }

    return [tool];
  });
};

const system = (skills: readonly Skill[], tools: readonly Tool[]): string =>
  [
    '# Objective execution',
    '',
    'Complete the user objective using only the available tools and the applicable skills below.',
    'Do not claim an action was performed unless a tool result confirms it.',
    'When the objective is complete, give a concise summary of the result.',
    '',
    '# Available skills',
    '',
    ...(skills.length === 0
      ? ['No skills are available.']
      : skills.flatMap((skill) => [
          `## ${skill.name}`,
          '',
          skill.description,
          '',
          `Allowed tools: ${skill.allowedTools.join(', ') || 'none'}.`,
          '',
          skill.body,
          '',
        ])),
    '# Available tools',
    '',
    ...(tools.length === 0
      ? ['No tools are available.']
      : tools.map(
          ({ name, description }) =>
            `- \`${name}\`: ${description ?? 'No description provided.'}`,
        )),
  ].join('\n');
