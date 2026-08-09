export interface Decision {
  readonly id: `D${string}`;
  readonly requirement: string;
  readonly testName: string;
}

const decision = (
  id: Decision['id'],
  requirement: string,
  testName: string,
): Decision => ({ id, requirement, testName });

export const DECISIONS: readonly Decision[] = [
  decision(
    'D01',
    'Catalog contains exactly 60 skills.',
    'D01 keeps exactly 60 micro-skills',
  ),
  decision(
    'D02',
    'Catalog contains 15 skills per domain.',
    'D02 balances 15 skills in every domain',
  ),
  decision(
    'D03',
    'All catalog relations and tool references resolve.',
    'D03 resolves every catalog annotation and tool reference',
  ),
  decision(
    'D04',
    'The deterministic menu has exactly the frozen 24 names.',
    'D04 exposes exactly the frozen 24 tool names',
  ),
  decision(
    'D05',
    'Every execution starts from an isolated world.',
    'D05 prevents world mutations from leaking across runs',
  ),
  decision(
    'D06',
    'Tool results are deterministic for equal world and input.',
    'D06 repeats deterministic tool results exactly',
  ),
  decision(
    'D07',
    'Pilot contains 60 unique cases and families.',
    'D07 keeps 60 unique pilot cases and families',
  ),
  decision(
    'D08',
    'Pilot contains ten valid cases per composition class.',
    'D08 enforces every composition class skill and tool count',
  ),
  decision(
    'D09',
    'Pilot contains 15 cases per domain.',
    'D09 balances 15 cases in every domain',
  ),
  decision(
    'D10',
    'Pilot and confirmatory families cannot overlap.',
    'D10 detects family leakage across study phases',
  ),
  decision(
    'D11',
    'Every condition passes ConditionV1.',
    'D11 validates every frozen condition contract',
  ),
  decision(
    'D12',
    'Each ablation changes only its declared factor.',
    'D12 proves the single-variable ablation matrix',
  ),
  decision(
    'D13',
    'M0 materializes unchanged feedback while M1 uses feedback.',
    'D13 distinguishes M0 unchanged feedback from M1',
  ),
  decision(
    'D14',
    'Oracles are failure-only.',
    'D14 rejects oracle scheduling without a failed parent',
  ),
  decision(
    'D15',
    'Seeded random streams reproduce exactly.',
    'D15 reproduces seeded random streams',
  ),
  decision(
    'D16',
    'Canonical hashes ignore object insertion order.',
    'D16 canonicalizes object keys before hashing',
  ),
  decision(
    'D17',
    'Concurrent event appends serialize sequence numbers.',
    'D17 serializes concurrent append-only events',
  ),
  decision(
    'D18',
    'Stored events form a verified hash chain.',
    'D18 verifies every event hash-chain link',
  ),
  decision(
    'D19',
    'Derived traces are immutable and content addressed.',
    'D19 derives the same immutable trace twice',
  ),
  decision(
    'D20',
    'Condition runs share order and deterministic-hook seeds inside case/repetition blocks; provider sampling is unseeded.',
    'D20 scopes paired seeds to condition order and deterministic hooks',
  ),
  decision(
    'D21',
    'A pre-model infrastructure failure permits one retry.',
    'D21 retries one technical failure before the first model call',
  ),
  decision(
    'D22',
    'A post-model interruption remains terminal.',
    'D22 never retries after the first model call',
  ),
  decision(
    'D23',
    'Baseline tie-breaks use success, cost, then lexical ID.',
    'D23 selects baseline with the frozen tie breakers',
  ),
  decision(
    'D24',
    'Freeze files never overwrite an existing manifest.',
    'D24 refuses to overwrite a frozen manifest',
  ),
  decision(
    'D25',
    'Six Doric smokes are opt-in and excluded from study phases.',
    'D25 defines one opt-in Doric smoke per class',
  ),
];
