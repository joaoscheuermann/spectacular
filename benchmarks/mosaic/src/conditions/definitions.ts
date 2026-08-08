import { ConditionV1, type Condition } from '../schemas/index.js';

const condition = (value: Omit<Condition, 'schemaVersion'>): Condition =>
  ConditionV1.parse({ schemaVersion: 1, ...value });

const m1Factors: Condition['factors'] = {
  decomposition: 'goal',
  catalogFeedback: true,
  skillView: 'body',
  retrieval: 'top-k',
  maxSkills: 3,
  bundle: 'selective',
  bundleOrder: 'ranked',
  menu: 'base-plus-bundle',
  baseTools: true,
  localizedRevision: true,
};

export const B0 = condition({
  id: 'B0',
  label: 'Single agent, global menu',
  description: 'One agent, no skills, and all 24 deterministic tools.',
  kind: 'baseline',
  eligibility: 'all',
  factors: {
    decomposition: 'none',
    catalogFeedback: false,
    skillView: 'none',
    retrieval: 'none',
    maxSkills: 0,
    bundle: 'none',
    bundleOrder: 'not-applicable',
    menu: 'global',
    baseTools: true,
    localizedRevision: false,
  },
  declaredChange: 'Reference baseline with no decomposition or skills.',
});

export const B1 = condition({
  id: 'B1',
  label: 'Single skill, global menu',
  description:
    'One body-aware skill for the complete request and all 24 tools.',
  kind: 'baseline',
  eligibility: 'all',
  factors: {
    decomposition: 'none',
    catalogFeedback: false,
    skillView: 'body',
    retrieval: 'top-k',
    maxSkills: 1,
    bundle: 'single',
    bundleOrder: 'not-applicable',
    menu: 'global',
    baseTools: true,
    localizedRevision: false,
  },
  declaredChange: 'Adds one whole-request skill to B0.',
});

export const B2 = condition({
  id: 'B2',
  label: 'Task planner, declared menu',
  description:
    'Task decomposition with exactly one body-aware skill and its declared tools per task.',
  kind: 'baseline',
  eligibility: 'all',
  factors: {
    decomposition: 'task',
    catalogFeedback: false,
    skillView: 'body',
    retrieval: 'top-k',
    maxSkills: 1,
    bundle: 'single',
    bundleOrder: 'not-applicable',
    menu: 'declared',
    baseTools: false,
    localizedRevision: false,
  },
  declaredChange: 'Adds task decomposition and skill-declared tool menus.',
});

export const B3 = condition({
  id: 'B3',
  label: 'Goal planner without feedback',
  description:
    'Goal plan, a fixed top-three body-aware bundle, base tools, and localized revision without P0-to-P1 feedback.',
  kind: 'baseline',
  eligibility: 'all',
  factors: { ...m1Factors, catalogFeedback: false, bundle: 'top-k' },
  declaredChange: 'MOSAIC-shaped baseline without catalog feedback.',
});

export const M0 = condition({
  id: 'M0',
  label: 'MOSAIC without catalog feedback',
  description:
    'MOSAIC engine with an unchanged P1 snapshot and no hint or P1 calls.',
  kind: 'mosaic',
  eligibility: 'all',
  factors: { ...m1Factors, catalogFeedback: false },
  declaredChange:
    'Disables catalog feedback while retaining selective routing and revision.',
});

export const M1 = condition({
  id: 'M1',
  label: 'MOSAIC 0.2',
  description: 'Complete MOSAIC 0.2 treatment.',
  kind: 'mosaic',
  eligibility: 'all',
  factors: m1Factors,
  declaredChange: 'Complete treatment reference.',
});

const ablation = (
  id: string,
  label: string,
  description: string,
  factors: Condition['factors'],
  declaredChange: string,
): Condition =>
  condition({
    id,
    label,
    description,
    kind: 'ablation',
    eligibility: 'all',
    factors,
    declaredChange,
  });

export const A1 = ablation(
  'A1',
  'Metadata-only skills',
  'Replaces complete skill bodies with metadata.',
  { ...m1Factors, skillView: 'metadata' },
  'skillView',
);
export const A2 = ablation(
  'A2',
  'Shuffled bundle order',
  'Keeps the selected skills but deterministically shuffles their order.',
  { ...m1Factors, bundleOrder: 'shuffled' },
  'bundleOrder',
);
export const A3 = ablation(
  'A3',
  'No base tools',
  'Removes Tbase while preserving the selected-skill tool contribution.',
  { ...m1Factors, baseTools: false },
  'baseTools',
);
export const A4 = ablation(
  'A4',
  'Fixed top-k bundle',
  'Replaces selective bundle choice with the fixed top-k retrieved skills.',
  { ...m1Factors, bundle: 'top-k' },
  'bundle',
);
export const A5 = ablation(
  'A5',
  'No localized revision',
  'Sets the localized-revision limit to zero.',
  { ...m1Factors, localizedRevision: false },
  'localizedRevision',
);

const oracle = (
  id: string,
  label: string,
  description: string,
  factors: Condition['factors'],
  oracleFor: string,
): Condition =>
  condition({
    id,
    label,
    description,
    kind: 'oracle',
    eligibility: 'failures-only',
    factors,
    declaredChange: `Diagnostic oracle for ${oracleFor}.`,
    oracleFor,
  });

export const ORACLES: readonly Condition[] = [
  oracle(
    'O_PLAN',
    'Oracle plan',
    'Injects the gold plan at the initial-plan boundary.',
    m1Factors,
    'M1',
  ),
  oracle(
    'O_RETRIEVAL',
    'Oracle retrieval',
    'Injects gold retrieval matches.',
    { ...m1Factors, retrieval: 'oracle' },
    'M1',
  ),
  oracle(
    'O_BUNDLE',
    'Oracle bundle',
    'Injects the gold routed bundle.',
    { ...m1Factors, bundle: 'oracle' },
    'M1',
  ),
  oracle(
    'O_MENU',
    'Oracle menu',
    'Injects the gold tool menu.',
    { ...m1Factors, menu: 'oracle' },
    'M1',
  ),
  oracle(
    'O_STATE',
    'Oracle state',
    'Injects the expected pre-execution world snapshot.',
    m1Factors,
    'M1',
  ),
  oracle(
    'O_REVISION',
    'Oracle revision',
    'Injects the gold localized revision.',
    m1Factors,
    'M1',
  ),
];

export const PRIMARY_CONDITIONS: readonly Condition[] = [
  B0,
  B1,
  B2,
  B3,
  M0,
  M1,
];
export const ABLATIONS: readonly Condition[] = [A1, A2, A3, A4, A5];
export const CONDITIONS: readonly Condition[] = [
  ...PRIMARY_CONDITIONS,
  ...ABLATIONS,
  ...ORACLES,
];

export const conditionById = (id: string): Condition => {
  const value = CONDITIONS.find((entry) => entry.id === id);
  if (value === undefined) throw new TypeError(`unknown condition: ${id}`);
  return value;
};
