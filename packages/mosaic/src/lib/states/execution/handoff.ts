import type { Graph, Node } from '../../types/graph.js';
import type { RevisionExecutionHandoff } from '../../types/revision.js';
import { revisionNodes } from '../revision/localized.js';

const observationLimit = 5;
const observationValueEdge = 400;

/**
 * Finds the direct retry, split, replacement, or merge lineage for this node,
 * falling back to its newest collateral plan change.
 */
export const revisionExecutionHandoff = (
  node: Node,
  graphs: readonly Graph[],
): RevisionExecutionHandoff | undefined => {
  let changedFallback: RevisionExecutionHandoff | undefined;

  for (let index = graphs.length - 1; index > 0; index -= 1) {
    const revised = graphs[index];
    const prior = graphs[index - 1];

    if (
      revised === undefined ||
      prior === undefined ||
      revised.revision <= 1 ||
      revised.revision !== prior.revision + 1
    ) {
      continue;
    }

    const revisedNode = revised.nodes.find(({ id }) => id === node.id);

    if (revisedNode === undefined) {continue;}

    const target = revisionNodes(prior)[0];

    if (target === undefined) {continue;}

    const priorNode = prior.nodes.find(({ id }) => id === node.id);
    const directlyReset = target.id === node.id;
    const introduced = priorNode === undefined;
    const changed = introduced || !samePlan(priorNode, revisedNode);

    if (!directlyReset && !changed) {continue;}

    const outcome = target.outcome;
    const request = outcome?.revisionRequest;

    if (outcome === null || request === null || request === undefined) {continue;}

    const falseCriteria = outcome.criteria.filter(
      ({ satisfied }) => !satisfied,
    );

    const citedLocalIds = new Set(
      falseCriteria.flatMap(({ observationIds }) => observationIds),
    );

    const citedObservations = target.observations.filter(({ id }) =>
      citedLocalIds.has(id),
    );
    const observations = citedObservations.slice(-observationLimit);

    const handoff = {
      invalidatedAssumption: request.invalidatedAssumption,
      requestedEffect: request.requestedEffect,
      falseCriteria: falseCriteria.map(({ criterionIndex }) => ({
        criterionIndex,
        text: target.doneWhen[criterionIndex],
      })),
      observations: observations.map(({ toolName, input, output }) => ({
        toolName,
        input: compactHistoricalValue(input),
        output: compactHistoricalValue(output),
      })),
      omittedObservationCount: citedObservations.length - observations.length,
    };
    const targetRetained = revised.nodes.some(({ id }) => id === target.id);
    const replacementOrMerge = !targetRetained && changed;

    // Direct retries and structural descendants outrank collateral pending edits.
    if (directlyReset || introduced || replacementOrMerge) {return handoff;}

    changedFallback ??= handoff;
  }

  return changedFallback;
};

const samePlan = (left: Node, right: Node): boolean =>
  left.id === right.id &&
  left.goal === right.goal &&
  left.deliver === right.deliver &&
  sameItems(left.doneWhen, right.doneWhen) &&
  sameItems(left.dependsOn, right.dependsOn);

const sameItems = (
  left: readonly string[],
  right: readonly string[],
): boolean =>
  left.length === right.length &&
  left.every((item, index) => item === right[index]);

const compactHistoricalValue = (value: string): string => {
  const retained = observationValueEdge * 2;

  if (value.length <= retained) {return value;}

  const omitted = value.length - retained;

  return [
    value.slice(0, observationValueEdge),
    `[${omitted} historical characters omitted]`,
    value.slice(-observationValueEdge),
  ].join('\n');
};
