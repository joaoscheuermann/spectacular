/*
Execute the already topologically ordered nodes sequentially, stopping at the first
unfinished node. Receives the runtime, search, request and nodes; invokes fresh
routing and execution for each node without forwarding earlier node results.
This deterministic scheduler has no model prompt or automatic replanning.
**/

import { executeNode } from './execute.mjs';
import { routeNode } from './route.mjs';

export const executePlan = async ({ runtime, search, request, nodes }) => {
  const results = [];
  const goals = nodes.map(({ goal }) => goal);

  for (const node of nodes) {
    const skills = await routeNode({ runtime, search, request, node });

    const result = await executeNode({
      runtime,
      request,
      goals,
      node,
      skills,
    });

    results.push(result);

    if (result.status !== 'completed') {
      break;
    }
  }

  return results;
};
