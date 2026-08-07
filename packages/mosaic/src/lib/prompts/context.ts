import type { Graph, Node } from '../types/graph.js';

export type ProjectedArtifact = {
  readonly producerId: string;
  readonly mime: string;
  readonly data: string;
};

/** Returns transitive-ancestor artifacts in stable graph and artifact order. */
export const projectedArtifacts = (
  node: Node,
  graph: Graph,
): readonly ProjectedArtifact[] => {
  const byId = new Map(
    graph.nodes.map((candidate) => [candidate.id, candidate]),
  );
  const ids = new Set<string>();

  const collect = (candidate: Node): void => {
    candidate.dependsOn.forEach((id) => {
      if (ids.has(id)) return;

      ids.add(id);
      const dependency = byId.get(id);
      if (dependency !== undefined) collect(dependency);
    });
  };

  collect(node);

  return graph.nodes
    .filter(({ id }) => ids.has(id))
    .flatMap((ancestor) =>
      ancestor.artifacts.map(({ mime, data }) => ({
        producerId: ancestor.id,
        mime,
        data,
      })),
    );
};

export const section = (heading: string, value: string): string =>
  `## ${heading}\n\n${fenced(value)}`;

export const fenced = (value: string): string => {
  const fence = selectFence(value);
  const body = value.endsWith('\n') ? value : `${value}\n`;
  return `${fence}text\n${body}${fence}`;
};

const selectFence = (value: string): string => {
  const backticks = longestRun(value, /`+/gu);
  const tildes = longestRun(value, /~+/gu);
  const marker = backticks <= tildes ? '`' : '~';
  const longest = marker === '`' ? backticks : tildes;

  return marker.repeat(Math.max(3, longest + 1));
};

const longestRun = (value: string, pattern: RegExp): number =>
  Math.max(0, ...[...value.matchAll(pattern)].map(([run]) => run.length));
