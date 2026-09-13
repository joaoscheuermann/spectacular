/*
Assemble accepted terminal-node Markdown and execution metrics without model calls.
Receives ordered planned nodes and executed results; omits delivery on an unfinished
run and reports the remaining node IDs.
**/

export const summarize = ({ nodes, results }) => {
  const completedNodes = results.filter(({ status }) => status === 'completed');
  const completed = completedNodes.length === nodes.length;
  const terminalResults = results.filter(({ deliver }) => deliver);

  const lastJudgments = results.map(
    (result) => result.attempts.at(-1).evaluation,
  );
  const evaluations = lastJudgments.flatMap(({ evaluations }) => evaluations);

  return {
    status: completed ? 'completed' : results.at(-1).status,
    nodes: results,
    unexecutedNodeIds: nodes.slice(results.length).map(({ id }) => id),
    delivery: completed
      ? terminalResults.map(({ result }) => result).join('\n\n')
      : null,
    metrics: {
      plannedNodes: nodes.length,
      completedNodes: completedNodes.length,
      attempts: results.reduce(
        (total, result) => total + result.attempts.length,
        0,
      ),
      criteria: nodes.reduce(
        (total, node) => total + node.acceptanceCriteria.length,
        0,
      ),
      satisfiedCriteria: evaluations.filter(({ satisfied }) => satisfied)
        .length,
    },
  };
};
