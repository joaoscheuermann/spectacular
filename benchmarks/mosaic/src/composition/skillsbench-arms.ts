import { createHash } from 'node:crypto';

import type { SkillsbenchCompositionManifest } from './skillsbench-catalog.js';

export type SkillsbenchCompositionArmId =
  | 'no-skills'
  | 'fixed-top-k'
  | 'mosaic-selective'
  | 'oracle'
  | 'all-skills';

export type SkillsbenchCompositionArm = {
  readonly id: SkillsbenchCompositionArmId;
  readonly diagnosticOnly: boolean;
  readonly selection:
    | { readonly kind: 'none' }
    | {
        readonly kind: 'fixed-top-k';
        readonly k: 3;
        readonly rankingSha256: string;
      }
    | { readonly kind: 'mosaic-selective'; readonly maxSkills: 8 }
    | { readonly kind: 'oracle' }
    | { readonly kind: 'all' };
};

export type SkillsbenchFixedRankingInput = {
  readonly catalogSha256: string;
  readonly ranker: {
    readonly id: string;
    readonly revision: string;
  };
  readonly tasks: readonly {
    readonly id: string;
    readonly skillIds: readonly string[];
  }[];
};

export type SkillsbenchFixedRanking = SkillsbenchFixedRankingInput & {
  readonly schemaVersion: 1;
  readonly sha256: string;
};

export type SkillsbenchCompositionContract = {
  readonly schemaVersion: 1;
  readonly condition: 'skillsbench-composition';
  readonly catalogSha256: string;
  readonly manifestSha256: string;
  readonly fixedRanking: SkillsbenchFixedRanking;
  readonly arms: readonly SkillsbenchCompositionArm[];
};

export type SkillsbenchArmAssignment =
  | { readonly kind: 'preloaded'; readonly skillIds: readonly string[] }
  | {
      readonly kind: 'selective';
      readonly candidateSkillIds: readonly string[];
      readonly maxSkills: 8;
    };

export type ResolveSkillsbenchCompositionArmOptions = {
  readonly contract: SkillsbenchCompositionContract;
  readonly manifest: SkillsbenchCompositionManifest;
  readonly arm: SkillsbenchCompositionArmId;
  readonly taskId: string;
};

const digest = (value: unknown): string =>
  createHash('sha256')
    .update(`skillsbench-fixed-ranking-v1\n${JSON.stringify(value)}`)
    .digest('hex');

const assertManifest = (manifest: SkillsbenchCompositionManifest): void => {
  const catalogIds = manifest.skills.map((skill) => skill.id);
  const taskIds = manifest.tasks.map((task) => task.id);

  if (new Set(catalogIds).size !== catalogIds.length)
    {throw new Error('SkillsBench catalog IDs must be unique.');}

  if (new Set(taskIds).size !== taskIds.length)
    {throw new Error('SkillsBench task IDs must be unique.');}

  const catalog = new Set(catalogIds);

  const invalid = manifest.tasks.find(
    (task) =>
      new Set(task.goldSkillIds).size !== task.goldSkillIds.length ||
      task.goldSkillIds.some((id) => !catalog.has(id)),
  );

  if (invalid !== undefined)
    {throw new Error(`Invalid gold skill association: ${invalid.id}`);}
};

const isCompletePermutation = (
  expected: readonly string[],
  actual: readonly string[],
): boolean => {
  if (expected.length !== actual.length) {return false;}

  const unique = new Set(actual);

  return (
    unique.size === actual.length && expected.every((id) => unique.has(id))
  );
};

const normalizeRanking = (
  manifest: SkillsbenchCompositionManifest,
  input: SkillsbenchFixedRankingInput,
): SkillsbenchFixedRanking => {
  if (input.catalogSha256 !== manifest.catalogSha256)
    {throw new Error(
      'Fixed ranking catalog digest does not match the manifest.',
    );}

  const ranker = {
    id: input.ranker.id.trim(),
    revision: input.ranker.revision.trim(),
  };

  if (ranker.id === '' || ranker.revision === '')
    {throw new Error('Fixed ranker provenance is required.');}

  const ids = manifest.tasks.map((task) => task.id);

  if (
    input.tasks.length !== ids.length ||
    new Set(input.tasks.map((task) => task.id)).size !== ids.length
  )
    {throw new Error('Fixed ranking requires one complete ranking per task.');}

  const byId = new Map(input.tasks.map((task) => [task.id, task.skillIds]));
  const catalogIds = manifest.skills.map((skill) => skill.id);

  const tasks = ids.map((id) => {
    const skillIds = byId.get(id);

    if (skillIds === undefined)
      {throw new Error('Fixed ranking requires one complete ranking per task.');}

    if (!isCompletePermutation(catalogIds, skillIds))
      {throw new Error(
        `Fixed ranking is not a complete catalog permutation: ${id}`,
      );}

    return { id, skillIds: [...skillIds] };
  });

  const unsigned = {
    schemaVersion: 1 as const,
    catalogSha256: input.catalogSha256,
    ranker,
    tasks,
  };

  return { ...unsigned, sha256: digest(unsigned) };
};

const arms = (rankingSha256: string): readonly SkillsbenchCompositionArm[] => [
  { id: 'no-skills', diagnosticOnly: false, selection: { kind: 'none' } },
  {
    id: 'fixed-top-k',
    diagnosticOnly: false,
    selection: { kind: 'fixed-top-k', k: 3, rankingSha256 },
  },
  {
    id: 'mosaic-selective',
    diagnosticOnly: false,
    selection: { kind: 'mosaic-selective', maxSkills: 8 },
  },
  { id: 'oracle', diagnosticOnly: true, selection: { kind: 'oracle' } },
  { id: 'all-skills', diagnosticOnly: true, selection: { kind: 'all' } },
];

/** Defines the closed, network-free composition treatment contract. */
export const defineSkillsbenchComposition = (
  manifest: SkillsbenchCompositionManifest,
  ranking: SkillsbenchFixedRankingInput,
): SkillsbenchCompositionContract => {
  assertManifest(manifest);

  const fixedRanking = normalizeRanking(manifest, ranking);

  return {
    schemaVersion: 1,
    condition: 'skillsbench-composition',
    catalogSha256: manifest.catalogSha256,
    manifestSha256: manifest.manifestSha256,
    fixedRanking,
    arms: arms(fixedRanking.sha256),
  };
};

const linked = (
  contract: SkillsbenchCompositionContract,
  manifest: SkillsbenchCompositionManifest,
): boolean =>
  contract.catalogSha256 === manifest.catalogSha256 &&
  contract.manifestSha256 === manifest.manifestSha256;

/** Resolves the skill exposure for one task without invoking a model or ranker. */
export const resolveSkillsbenchCompositionArm = (
  options: ResolveSkillsbenchCompositionArmOptions,
): SkillsbenchArmAssignment => {
  const { contract, manifest, arm, taskId } = options;

  if (!linked(contract, manifest))
    {throw new Error(
      'Composition contract does not match the catalog manifest.',
    );}

  const task = manifest.tasks.find((entry) => entry.id === taskId);

  if (task === undefined)
    {throw new Error(`Unknown SkillsBench task: ${taskId}`);}

  const definition = contract.arms.find((entry) => entry.id === arm);

  if (definition === undefined)
    {throw new Error(`Unknown composition arm: ${arm}`);}

  const catalogIds = manifest.skills.map((skill) => skill.id);

  if (definition.selection.kind === 'none')
    {return { kind: 'preloaded', skillIds: [] };}

  if (definition.selection.kind === 'oracle')
    {return { kind: 'preloaded', skillIds: [...task.goldSkillIds] };}

  if (definition.selection.kind === 'all')
    {return { kind: 'preloaded', skillIds: catalogIds };}

  if (definition.selection.kind === 'mosaic-selective')
    {return {
      kind: 'selective',
      candidateSkillIds: catalogIds,
      maxSkills: definition.selection.maxSkills,
    };}

  const ranking = contract.fixedRanking.tasks.find(
    (entry) => entry.id === taskId,
  );

  if (ranking === undefined)
    {throw new Error(`Fixed ranking is missing task: ${taskId}`);}

  return {
    kind: 'preloaded',
    skillIds: ranking.skillIds.slice(0, definition.selection.k),
  };
};
