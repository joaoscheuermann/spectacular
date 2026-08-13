import { PlanningCaseSchema, type PlanningCase } from '../planning-schema.js';
import { artifactCases } from './artifacts.js';
import { communicationCases } from './communications.js';
import { documentFinanceCases } from './documents-finance.js';
import { softwareCases } from './software.js';

const parsed = PlanningCaseSchema.array()
  .length(24)
  .parse([
    ...documentFinanceCases,
    ...softwareCases,
    ...artifactCases,
    ...communicationCases,
  ]);

/** Frozen 6-class by 4-domain controlled P0-to-P1 planning corpus. */
export const planningCases: readonly PlanningCase[] = Object.freeze(parsed);

export const planningCase = (id: string): PlanningCase => {
  const value = planningCases.find((candidate) => candidate.id === id);
  if (value === undefined) throw new TypeError(`Unknown planning case: ${id}`);
  return value;
};
