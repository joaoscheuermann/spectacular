import type { Artifact } from '../types/artifact.js';
import type { Graph, Node } from '../types/graph.js';
import type { CriterionEvaluation, NodeOutcome } from '../schemas/outcome.js';
import type { Observation } from '../schemas/observation.js';
import { completedAncestors, projectedObservations } from '../observations.js';

export type ProjectedArtifact = Artifact & {
  readonly producerId: string;
};

export type ProjectedAncestor = {
  readonly producerId: string;
  readonly status: 'completed';
  readonly criteria: readonly CriterionEvaluation[];
  readonly observationCount: number;
  readonly citedObservationCount: number;
  readonly observations: readonly Observation[];
  readonly artifacts: readonly Artifact[];
};

/** Returns completed transitive ancestors with only criterion-cited evidence. */
export const projectedAncestors = (
  node: Node,
  graph: Graph,
): readonly ProjectedAncestor[] =>
  completedAncestors(node, graph).map((ancestor) => {
    const outcome = completedOutcome(ancestor);
    const cited = new Set(
      outcome.criteria.flatMap(({ observationIds }) => observationIds),
    );
    const observations = ancestor.observations.filter(({ id }) =>
      cited.has(id),
    );

    return {
      producerId: ancestor.id,
      status: 'completed',
      criteria: outcome.criteria,
      observationCount: ancestor.observations.length,
      citedObservationCount: observations.length,
      observations,
      artifacts: ancestor.artifacts,
    };
  });

/** Returns causally projected observation objects once in producer-ledger order. */
export const projectedObservationLedger = (
  node: Node,
  graph: Graph,
): readonly {
  readonly producerId: string;
  readonly observation: Observation;
}[] =>
  projectedObservations(node, graph).map((observation) => ({
    producerId: observation.goalId,
    observation,
  }));

/** Returns transitive-ancestor artifacts in stable graph and artifact order. */
export const projectedArtifacts = (
  node: Node,
  graph: Graph,
): readonly ProjectedArtifact[] =>
  completedAncestors(node, graph).flatMap((ancestor) =>
    ancestor.artifacts.map((artifact) => ({
      ...artifact,
      producerId: ancestor.id,
    })),
  );

const completedOutcome = (node: Node): NodeOutcome => {
  if (node.outcome?.status === 'completed') return node.outcome;
  throw new Error(`Completed ancestor ${node.id} is missing its outcome.`);
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
