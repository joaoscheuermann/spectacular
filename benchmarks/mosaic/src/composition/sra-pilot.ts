import { createHash } from 'node:crypto';

import type { SraCorpusSkill, SraInstance } from './sra-fixtures.js';

/** Immutable provenance and selection contract for the external SRA-Bench pilot. */
export const SRA_BENCH_MANIFEST = {
  schemaVersion: 1,
  benchmark: 'SRA-Bench',
  upstream: {
    repository: 'https://github.com/oneal2000/SR-Agents',
    revision: '277fd8d2bbd7d3b81a5cf4ffa6e87e18c7906e4f',
    paper: 'https://arxiv.org/abs/2604.24594',
    license: 'MIT',
  },
  dataset: {
    repository: 'https://huggingface.co/datasets/WeihangSu/SRA-Bench',
    revision: '6143f2634eb284955ce312213bac24b582d039f3',
    license: 'MIT',
    artifacts: {
      corpus: {
        path: 'corpus/corpus.json',
        bytes: 232_255_987,
        sha256:
          '16ee509ae5bea8c2e17167dffecd89100a7d8dfa31256c3742426758c7169b5e',
      },
      instances: [
        {
          dataset: 'champ',
          path: 'instances/champ.json',
          bytes: 72_375,
          sha256:
            'd61346716cede953afb352e739e170b96d2bfb98824edd91b8783dc3526c7cec',
          records: 223,
          eligibleMultiSkillRecords: 109,
          cardinalityCounts: { 1: 114, 2: 69, 3: 31, 4: 8, 5: 1 },
        },
        {
          dataset: 'bigcodebench',
          path: 'instances/bigcodebench.json',
          bytes: 4_623_115,
          sha256:
            '0ed01363e2c93134cf8696fea47b6d640f32f0d7ae9d7b478a05595c3f5ae788',
          records: 1_140,
          eligibleMultiSkillRecords: 1_140,
          cardinalityCounts: { 2: 481, 3: 475, 4: 159, 5: 22, 6: 3 },
        },
      ],
    },
  },
  pilot: {
    datasets: ['champ', 'bigcodebench'],
    minimumGoldSkills: 2,
    perDataset: 50,
    totalInstances: 100,
    strata: [
      { dataset: 'champ', cardinality: 2, count: 31 },
      { dataset: 'champ', cardinality: 3, count: 14 },
      { dataset: 'champ', cardinality: 4, count: 4 },
      { dataset: 'champ', cardinality: 5, count: 1 },
      { dataset: 'bigcodebench', cardinality: 2, count: 21 },
      { dataset: 'bigcodebench', cardinality: 3, count: 20 },
      { dataset: 'bigcodebench', cardinality: 4, count: 7 },
      { dataset: 'bigcodebench', cardinality: 5, count: 1 },
      { dataset: 'bigcodebench', cardinality: 6, count: 1 },
    ],
    selection: 'lowest-sha256-within-cardinality',
    seed: 'sra-bench-pilot-v1:6143f2634eb284955ce312213bac24b582d039f3',
  },
} as const;

export type SraPilotDataset =
  (typeof SRA_BENCH_MANIFEST.pilot.datasets)[number];

export type SraSkillGold = {
  readonly instance_id: string;
  readonly dataset: SraPilotDataset;
  readonly gold_skill_ids: readonly string[];
};

const compareText = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;

const isPilotDataset = (dataset: string): dataset is SraPilotDataset =>
  SRA_BENCH_MANIFEST.pilot.datasets.some((candidate) => candidate === dataset);

const isEligible = (instance: SraInstance): boolean =>
  isPilotDataset(instance.dataset) &&
  new Set(instance.skill_annotations).size >=
    SRA_BENCH_MANIFEST.pilot.minimumGoldSkills;

const selectionScore = (instance: SraInstance): string =>
  createHash('sha256')
    .update(
      `${SRA_BENCH_MANIFEST.pilot.seed}\n${instance.dataset}\n${instance.instance_id}`,
    )
    .digest('hex');

type SraPilotStratum = (typeof SRA_BENCH_MANIFEST.pilot.strata)[number];

const selectStratum = (
  instances: readonly SraInstance[],
  stratum: SraPilotStratum,
): readonly SraInstance[] => {
  const eligible = instances.filter(
    (instance) =>
      instance.dataset === stratum.dataset &&
      instance.skill_annotations.length === stratum.cardinality &&
      isEligible(instance),
  );

  if (eligible.length < stratum.count) {
    throw new Error(
      `SRA-Bench pilot requires ${stratum.count} eligible ${stratum.dataset} cardinality-${stratum.cardinality} instances; found ${eligible.length}.`,
    );
  }

  return eligible
    .map((instance) => ({ instance, score: selectionScore(instance) }))
    .sort(
      (left, right) =>
        compareText(left.score, right.score) ||
        compareText(left.instance.instance_id, right.instance.instance_id),
    )
    .slice(0, stratum.count)
    .map(({ instance }) => instance);
};

/** Selects the fixed balanced pilot independently of local fixture ordering. */
export const selectSraPilot = (
  instances: readonly SraInstance[],
): readonly SraInstance[] =>
  SRA_BENCH_MANIFEST.pilot.strata.flatMap((stratum) =>
    selectStratum(instances, stratum),
  );

const canonicalGoldSkillIds = (instance: SraInstance): readonly string[] =>
  [...new Set(instance.skill_annotations)].sort(compareText);

function assertPilotInstance(
  instance: SraInstance,
): asserts instance is SraInstance & { readonly dataset: SraPilotDataset } {
  if (!isPilotDataset(instance.dataset) || !isEligible(instance)) {
    throw new Error(
      `SRA-Bench pilot instance ${instance.instance_id} is not an eligible multi-skill CHAMP or BigCodeBench query.`,
    );
  }
}

/** Builds canonical skill gold sets and verifies every annotation is in corpus. */
export const buildSraSkillGolds = (
  pilot: readonly SraInstance[],
  corpus: readonly SraCorpusSkill[],
): readonly SraSkillGold[] => {
  const corpusSkillIds = new Set(corpus.map((skill) => skill.skill_id));

  return pilot.map((instance) => {
    assertPilotInstance(instance);

    const goldSkillIds = canonicalGoldSkillIds(instance);

    const missing = goldSkillIds.find(
      (skillId) => !corpusSkillIds.has(skillId),
    );

    if (missing) {
      throw new Error(
        `SRA-Bench instance ${instance.instance_id} references missing corpus skill ${missing}.`,
      );
    }

    return {
      instance_id: instance.instance_id,
      dataset: instance.dataset,
      gold_skill_ids: goldSkillIds,
    };
  });
};
