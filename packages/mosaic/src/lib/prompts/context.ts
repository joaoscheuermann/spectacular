import type { Artifact } from '../types/artifact.js';
import type { Graph, Node } from '../types/graph.js';

export type ProjectedArtifact = Artifact & {
  readonly producerId: string;
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
      ancestor.artifacts.map((artifact) => ({
        ...artifact,
        producerId: ancestor.id,
      })),
    );
};

export const section = (heading: string, value: string): string =>
  `## ${heading}\n\n${fenced(value)}`;

/** Renders one artifact without conflating opaque references with inline data. */
export const artifactSections = (artifact: Artifact): string[] => [
  section('Kind', artifact.kind),
  section('MIME Type', artifact.mime),
  artifact.kind === 'inline'
    ? section('Data', artifact.data)
    : section('Reference', artifact.reference),
];

/** Renders planner-owned graph fields with every dynamic value fenced. */
export const graphContext = (graph: Graph): string =>
  [
    '# Active Plan',
    section('Revision', String(graph.revision)),
    ...graph.nodes.flatMap((node, index) => [
      `## Node ${index}`,
      section('ID', node.id),
      section('Goal', node.goal),
      '## Completion Criteria',
      ...node.doneWhen.flatMap((criterion, criterionIndex) => [
        `### Criterion ${criterionIndex}`,
        fenced(criterion),
      ]),
      '## Dependencies',
      ...(node.dependsOn.length === 0
        ? ['No dependencies.']
        : node.dependsOn.flatMap((dependency, dependencyIndex) => [
            `### Dependency ${dependencyIndex}`,
            fenced(dependency),
          ])),
      section('Deliver', String(node.deliver)),
      section('Runtime Status', node.status),
    ]),
  ].join('\n\n');

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
