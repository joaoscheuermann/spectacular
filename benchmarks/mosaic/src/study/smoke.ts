import {
  CaseV1,
  ConditionV1,
  type Case,
  type Condition,
} from '../schemas/index.js';
import { M1 } from '../conditions/index.js';
import { artifactHash } from '../core/hash.js';
import { PILOT_CASES } from './cases.js';

const CLASSES = ['A', 'B', 'C', 'D', 'E', 'F'] as const;

const smokeCase = (compositionClass: (typeof CLASSES)[number]): Case => {
  const source = PILOT_CASES.find(
    (entry) => entry.compositionClass === compositionClass,
  );
  if (source === undefined)
    throw new Error(`missing pilot source for class ${compositionClass}`);
  const { contentHash: ignored, ...sourceBody } = source;
  void ignored;
  const body = {
    ...sourceBody,
    id: `doric.smoke.case.${compositionClass}`,
    familyId: `doric.smoke.family.${compositionClass}`,
    phase: 'smoke' as const,
    title: `Doric opt-in smoke ${compositionClass}`,
    tags: ['smoke', 'doric', 'opt-in', `class.${compositionClass}`],
  };
  return CaseV1.parse({ ...body, contentHash: artifactHash(body) });
};

const smokeCondition = (
  compositionClass: (typeof CLASSES)[number],
): Condition =>
  ConditionV1.parse({
    schemaVersion: 1,
    id: `DORIC_SMOKE_${compositionClass}`,
    label: `Doric smoke ${compositionClass}`,
    description: `Opt-in Doric-targeted contract smoke for class ${compositionClass}; excluded from benchmark analysis.`,
    kind: 'smoke',
    eligibility: 'opt-in',
    factors: M1.factors,
    declaredChange:
      'No experimental change; composition-root integration only.',
  });

/** Exactly six opt-in Doric-targeted contracts, one for each class. */
export const DORIC_SMOKE_CASES: readonly Case[] = CLASSES.map(smokeCase);
export const DORIC_SMOKE_CONDITIONS: readonly Condition[] =
  CLASSES.map(smokeCondition);
