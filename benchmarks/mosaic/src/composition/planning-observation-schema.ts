import {
  PlanningCaseSchema,
  PlanningObservationSchema,
  type PlanningCase,
} from './planning-schema.js';

/** Binds semantic observation labels to one validated planning case catalog. */
export const planningObservationSchema = (input: PlanningCase) => {
  const benchmarkCase = PlanningCaseSchema.parse(input);
  const labels = [
    ['roleIds', new Set(benchmarkCase.gold.roles.map(({ id }) => id)), 'Role'],
    [
      'outputIds',
      new Set(benchmarkCase.gold.outputs.map(({ id }) => id)),
      'Output',
    ],
    [
      'behaviorIds',
      new Set(benchmarkCase.gold.behaviors.map(({ id }) => id)),
      'Behavior',
    ],
  ] as const;

  return PlanningObservationSchema.superRefine((value, context) => {
    value.nodes.forEach((node, nodeIndex) => {
      labels.forEach(([field, known, label]) => {
        node[field].forEach((id, labelIndex) => {
          if (known.has(id)) return;
          context.addIssue({
            code: 'custom',
            path: ['nodes', nodeIndex, field, labelIndex],
            message: `${label} ID must appear in the supplied catalog.`,
          });
        });
      });
    });
  });
};
