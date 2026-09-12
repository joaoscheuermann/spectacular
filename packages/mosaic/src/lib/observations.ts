import type { Observation } from './schemas/observation.js';
import type { Graph, Node } from './types/graph.js';

/** Returns completed transitive ancestors in stable active-graph order. */
export const completedAncestors = (
  node: Node,
  graph: Graph,
): readonly Node[] => {
  const byId = new Map(
    graph.nodes.map((candidate) => [candidate.id, candidate]),
  );
  const ids = new Set<string>();

  const collect = (candidate: Node): void => {
    candidate.dependsOn.forEach((id) => {
      if (ids.has(id)) {return;}

      ids.add(id);

      const dependency = byId.get(id);

      if (dependency !== undefined) {collect(dependency);}
    });
  };

  collect(node);

  return graph.nodes.filter(
    (candidate) => ids.has(candidate.id) && candidate.status === 'completed',
  );
};

/** Returns the full ancestor observations cited by completed transitive ancestors. */
export const projectedObservations = (
  node: Node,
  graph: Graph,
): readonly Observation[] => {
  const ancestors = completedAncestors(node, graph);

  const cited = new Set(
    ancestors.flatMap(
      (ancestor) =>
        ancestor.outcome?.criteria.flatMap(
          ({ observationIds }) => observationIds,
        ) ?? [],
    ),
  );
  const ancestorIds = new Set(ancestors.map(({ id }) => id));

  return graph.nodes.flatMap((producer) =>
    ancestorIds.has(producer.id)
      ? producer.observations.filter(({ id }) => cited.has(id))
      : [],
  );
};

/** Returns every observation ID presented to the executing node. */
export const authorizedObservationIds = (
  node: Node,
  graph: Graph,
): ReadonlySet<string> =>
  new Set([
    ...node.observations.map(({ id }) => id),
    ...projectedObservations(node, graph).map(({ id }) => id),
  ]);
