import type { Case } from '../schemas/index.js';
import { CaseV1 } from '../schemas/index.js';
import { SKILLS, type MicroSkill } from '../catalog/index.js';
import { BASE_TOOL_NAMES, TOOL_NAMES, type ToolName } from '../config/index.js';
import { artifactHash } from '../core/hash.js';
import type { JsonValue } from '../core/json.js';
import { executeTool } from '../runtime/tools.js';
import { createWorld, worldHash } from '../runtime/world.js';

const CLASSES = ['A', 'B', 'C', 'D', 'E', 'F'] as const;
const MUTATING_TOOLS: readonly ToolName[] = [
  'write',
  'artifact_publish',
  'message_send',
];

export interface CaseIssue {
  readonly code: string;
  readonly caseId: string;
  readonly detail: string;
}

const validClassComposition = (entry: Case): boolean => {
  const skills = entry.gold.requiredSkills.length;
  const tools = entry.gold.requiredTools.length;
  const baseOnly = entry.gold.requiredTools.every((tool) =>
    (BASE_TOOL_NAMES as readonly string[]).includes(tool),
  );
  const rules: Readonly<Record<Case['compositionClass'], boolean>> = {
    A: skills === 0 && tools === 0,
    B: skills === 0 && tools >= 1 && baseOnly,
    C: skills >= 1 && tools === 0,
    D: skills === 1 && tools >= 1,
    E: skills >= 2 && tools <= 1,
    F: skills >= 2 && tools >= 2,
  };
  return rules[entry.compositionClass];
};

const validExpectedExecution = (entry: Case): boolean => {
  const expected = entry.gold.expectedState;
  if (expected.fixtureId !== 'world-v1') return false;
  if (
    expected.toolEvidence.map(({ name }) => name).join() !==
    entry.gold.requiredTools.join()
  )
    return false;
  let world = createWorld();
  try {
    for (const evidence of expected.toolEvidence) {
      if (!(TOOL_NAMES as readonly string[]).includes(evidence.name))
        return false;
      const execution = executeTool(
        world,
        evidence.name as ToolName,
        evidence.input as JsonValue,
      );
      if (
        artifactHash({
          name: evidence.name,
          input: evidence.input,
          output: execution.output,
        }) !== evidence.evidenceHash
      )
        return false;
      world = execution.world;
    }
  } catch {
    return false;
  }
  const effects = entry.gold.requiredTools.filter((tool) =>
    MUTATING_TOOLS.includes(tool as ToolName),
  );
  return (
    worldHash(world) === expected.worldHash &&
    effects.join() === expected.requiredEffects.join()
  );
};

const independentFingerprint = (entry: Case): string =>
  artifactHash({
    title: entry.title,
    domain: entry.domain,
    compositionClass: entry.compositionClass,
    focusGoalRole: entry.focusGoalRole,
    composition: entry.composition,
    adaptive: entry.adaptive,
    request: entry.request,
    fixtureIds: entry.fixtureIds,
    gold: entry.gold,
  });

/** Checks balance, catalog references, hashes, and cross-phase family leakage. */
export const validateCases = (
  cases: readonly Case[],
  skills: readonly MicroSkill[] = SKILLS,
): readonly CaseIssue[] => {
  const skillIds = new Set(skills.map((skill) => skill.id));
  const issues: CaseIssue[] = [];
  const uniqueCases = new Set(cases.map((entry) => entry.id));
  const uniqueFamilies = new Set(cases.map((entry) => entry.familyId));

  if (
    cases.length !== 60 ||
    uniqueCases.size !== 60 ||
    uniqueFamilies.size !== 60
  ) {
    issues.push({
      code: 'pilot_cardinality',
      caseId: '$',
      detail: 'expected 60 unique cases and families',
    });
  }

  cases.forEach((entry) => {
    const { contentHash: ignored, ...body } = entry;
    void ignored;
    if (artifactHash(body) !== entry.contentHash) {
      issues.push({
        code: 'content_hash',
        caseId: entry.id,
        detail: 'content hash mismatch',
      });
    }
    const references = [
      ...entry.gold.requiredSkills,
      ...entry.gold.relevantSkills,
      ...entry.gold.forbiddenSkills,
    ];
    references
      .filter((id) => !skillIds.has(id))
      .forEach((id) => {
        issues.push({ code: 'unknown_skill', caseId: entry.id, detail: id });
      });
    const expectedSkillCount =
      entry.gold.requiredSkills.length === 0
        ? 0
        : entry.gold.requiredSkills.length === 1
          ? 1
          : 'many';
    const expectedToolCount =
      entry.gold.requiredTools.length === 0
        ? 0
        : entry.gold.requiredTools.length === 1
          ? 1
          : 'many';
    if (
      entry.composition.skillCount !== expectedSkillCount ||
      entry.composition.toolCount !== expectedToolCount
    ) {
      issues.push({
        code: 'composition_signature',
        caseId: entry.id,
        detail: 'gold counts disagree with composition signature',
      });
    }
    if (entry.composition.requiresRevision !== entry.gold.requiresRevision) {
      issues.push({
        code: 'revision_signature',
        caseId: entry.id,
        detail: 'gold revision disagrees with composition signature',
      });
    }
    if (
      entry.gold.requiredSkills.some((id) =>
        entry.gold.forbiddenSkills.includes(id),
      )
    ) {
      issues.push({
        code: 'skill_gold_conflict',
        caseId: entry.id,
        detail: 'a required skill is also forbidden',
      });
    }
    if (
      entry.gold.requiredSkills.length === 0 &&
      entry.gold.forbiddenSkills.length !== skills.length
    ) {
      issues.push({
        code: 'empty_bundle_gold',
        caseId: entry.id,
        detail: 'zero-skill cases must forbid the complete catalog',
      });
    }
    if (
      entry.gold.requiresRevision &&
      (entry.gold.requiredTools.length === 0 ||
        !entry.request.includes('structural invalidation'))
    ) {
      issues.push({
        code: 'revision_observation',
        caseId: entry.id,
        detail: 'revision requires an explicit tool observation trigger',
      });
    }
    if (!validExpectedExecution(entry)) {
      issues.push({
        code: 'expected_execution',
        caseId: entry.id,
        detail: 'tool evidence or final world hash is not reproducible',
      });
    }
    if (!validClassComposition(entry)) {
      issues.push({
        code: 'composition_class',
        caseId: entry.id,
        detail: `invalid ${entry.compositionClass} skill/tool counts`,
      });
    }
  });

  const dimensions = [
    ...(
      ['documents-finance', 'software', 'artifacts', 'communication'] as const
    ).map((domain) => ({
      id: domain,
      expected: 15,
      count: cases.filter((entry) => entry.domain === domain).length,
    })),
    ...CLASSES.map((value) => ({
      id: value,
      expected: 10,
      count: cases.filter((entry) => entry.compositionClass === value).length,
    })),
  ];
  dimensions
    .filter((item) => item.count !== item.expected)
    .forEach((item) => {
      issues.push({
        code: 'unbalanced_dimension',
        caseId: '$',
        detail: `${item.id}:${item.count}`,
      });
    });
  return issues;
};

/** Validates a frozen confirmatory corpus against the 6-by-4 balanced design. */
export const validateConfirmatoryCases = (
  cases: readonly Case[],
  pilot: readonly Case[],
  nFinal: number,
  skills: readonly MicroSkill[] = SKILLS,
): readonly CaseIssue[] => {
  const issues = validateCases(cases, skills).filter(
    ({ code }) =>
      code !== 'pilot_cardinality' && code !== 'unbalanced_dimension',
  );
  if (!Number.isSafeInteger(nFinal) || nFinal < 240 || nFinal % 120 !== 0) {
    issues.push({
      code: 'confirmatory_n_final',
      caseId: '$',
      detail: 'nFinal must be at least 240 and a multiple of 120',
    });
    return issues;
  }
  if (cases.length !== nFinal) {
    issues.push({
      code: 'confirmatory_cardinality',
      caseId: '$',
      detail: `expected ${nFinal} cases`,
    });
  }
  if (new Set(cases.map(({ id }) => id)).size !== cases.length) {
    issues.push({
      code: 'confirmatory_case_id',
      caseId: '$',
      detail: 'confirmatory case IDs must be unique',
    });
  }
  if (new Set(cases.map(({ familyId }) => familyId)).size !== cases.length) {
    issues.push({
      code: 'confirmatory_family_id',
      caseId: '$',
      detail: 'confirmatory family IDs must be unique',
    });
  }
  const pilotFamilies = new Set(pilot.map(({ familyId }) => familyId));
  const pilotFingerprints = new Set(pilot.map(independentFingerprint));
  cases
    .filter(({ familyId }) => pilotFamilies.has(familyId))
    .forEach((entry) => {
      issues.push({
        code: 'family_leakage',
        caseId: entry.id,
        detail: entry.familyId,
      });
    });
  cases
    .filter((entry) => pilotFingerprints.has(independentFingerprint(entry)))
    .forEach((entry) => {
      issues.push({
        code: 'pilot_content_leakage',
        caseId: entry.id,
        detail: 'exact non-identity content matches a pilot case',
      });
    });
  cases
    .filter(({ phase }) => phase !== 'confirmatory')
    .forEach((entry) => {
      issues.push({
        code: 'confirmatory_phase',
        caseId: entry.id,
        detail: entry.phase,
      });
    });
  cases
    .filter((entry) => !CaseV1.safeParse(entry).success)
    .forEach((entry) => {
      issues.push({
        code: 'confirmatory_schema',
        caseId: entry.id,
        detail: 'CaseV1 validation failed',
      });
    });
  const perCell = nFinal / 24;
  for (const compositionClass of CLASSES) {
    for (const domain of [
      'documents-finance',
      'software',
      'artifacts',
      'communication',
    ] as const) {
      const count = cases.filter(
        (entry) =>
          entry.compositionClass === compositionClass &&
          entry.domain === domain,
      ).length;
      if (count !== perCell) {
        issues.push({
          code: 'confirmatory_cell',
          caseId: '$',
          detail: `${compositionClass}:${domain}:${count}/${perCell}`,
        });
      }
    }
  }
  return issues;
};

/** Rejects any family reused across independent study phases. */
export const validateFamilyIsolation = (
  pilot: readonly Case[],
  confirmatory: readonly Case[],
): readonly CaseIssue[] => {
  const pilotFamilies = new Set(pilot.map((entry) => entry.familyId));
  const pilotFingerprints = new Set(pilot.map(independentFingerprint));
  return confirmatory.flatMap((entry) => {
    if (pilotFamilies.has(entry.familyId)) {
      return [
        { code: 'family_leakage', caseId: entry.id, detail: entry.familyId },
      ];
    }
    return pilotFingerprints.has(independentFingerprint(entry))
      ? [
          {
            code: 'pilot_content_leakage',
            caseId: entry.id,
            detail: 'exact non-identity content matches a pilot case',
          },
        ]
      : [];
  });
};
