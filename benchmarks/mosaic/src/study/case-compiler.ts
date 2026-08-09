import {
  CaseDraftDocumentV1,
  CaseV1,
  type Case,
  type CaseDraft,
} from '../schemas/index.js';
import { SKILLS } from '../catalog/index.js';
import { BASE_TOOL_NAMES, TOOL_NAMES, type ToolName } from '../config/index.js';
import { artifactHash } from '../core/hash.js';
import { jsonSnapshot, type JsonValue } from '../core/json.js';
import { executeTool } from '../runtime/tools.js';
import { createWorld, worldHash } from '../runtime/world.js';
import { validateCalibrationCases, type CaseIssue } from './case-validation.js';

const MUTATING_TOOLS: readonly ToolName[] = [
  'write',
  'artifact_publish',
  'message_send',
];

export interface CaseCompilationReport {
  readonly schemaVersion: 1;
  readonly valid: boolean;
  readonly cases: readonly Case[];
  readonly issues: readonly CaseIssue[];
  readonly humanAuditRequired: readonly [
    'semantic-neutrality',
    'cross-phase-independence',
  ];
}

const humanAuditRequired = [
  'semantic-neutrality',
  'cross-phase-independence',
] as const;

const schemaIssues = (input: unknown): readonly CaseIssue[] => {
  const parsed = CaseDraftDocumentV1.safeParse(input);
  if (parsed.success) return [];
  return parsed.error.issues.map((issue) => ({
    code: 'draft_schema',
    caseId: typeof issue.path[1] === 'number' ? `$[${issue.path[1]}]` : '$',
    detail: `${issue.path.join('.') || '$'}: ${issue.message}`,
  }));
};

const toolRequirement = (
  names: readonly ToolName[],
): Case['composition']['toolRequirement'] => {
  if (names.length === 0) return 'none';
  const base = names.filter((name) =>
    (BASE_TOOL_NAMES as readonly string[]).includes(name),
  ).length;
  if (base === names.length) return 'base';
  return base === 0 ? 'declared' : 'mixed';
};

const count = (values: readonly unknown[]): 0 | 1 | 'many' =>
  values.length === 0 ? 0 : values.length === 1 ? 1 : 'many';

const compileOne = (
  draft: CaseDraft,
): { readonly case?: Case; readonly issues: readonly CaseIssue[] } => {
  let world = createWorld();
  const observations: Record<string, JsonValue> = {};
  const toolEvidence = [];
  const names: ToolName[] = [];
  const issues: CaseIssue[] = [];

  for (const tool of draft.tools) {
    if (!(TOOL_NAMES as readonly string[]).includes(tool.name)) {
      issues.push({
        code: 'unknown_tool',
        caseId: draft.id,
        detail: tool.name,
      });
      continue;
    }
    try {
      const name = tool.name as ToolName;
      const input = tool.input as JsonValue;
      const execution = executeTool(world, name, input);
      world = execution.world;
      names.push(name);
      observations[tool.id] = execution.output;
      toolEvidence.push({
        id: tool.id,
        name,
        input,
        evidenceHash: artifactHash({ name, input, output: execution.output }),
      });
    } catch {
      issues.push({
        code: 'tool_execution',
        caseId: draft.id,
        detail: tool.id,
      });
    }
  }
  if (issues.length > 0) return { issues };

  const document = jsonSnapshot({
    fixtureId: world.fixtureId,
    answer: draft.expectedAnswer,
    observations,
  } as JsonValue) as Readonly<Record<string, JsonValue>>;
  const fields = [
    { id: 'delivery.fixture', path: ['fixtureId'] },
    { id: 'delivery.answer', path: ['answer'] },
    ...draft.tools.map((tool) => ({
      id: `delivery.${tool.id}`,
      path: ['observations', tool.id],
    })),
  ];
  const required = new Set(names);
  const forbiddenSkills =
    draft.requiredSkills.length === 0
      ? SKILLS.map(({ id }) => id)
      : draft.forbiddenSkills;
  const body = {
    schemaVersion: 1 as const,
    id: draft.id,
    familyId: draft.familyId,
    phase: draft.phase,
    title: draft.title,
    domain: draft.domain,
    compositionClass: draft.compositionClass,
    focusGoalRole: draft.focusGoalRole,
    composition: {
      skillCount: count(draft.requiredSkills),
      toolRequirement: toolRequirement(names),
      toolCount: count(names),
      requiresRevision: draft.requiresRevision,
      requiresExternalEffect: names.some((name) =>
        MUTATING_TOOLS.includes(name),
      ),
    },
    adaptive: draft.adaptive,
    request: draft.request,
    fixtureIds: draft.fixtureIds,
    tags: draft.tags,
    gold: {
      criteria: draft.criteria,
      requiredSkills: draft.requiredSkills,
      relevantSkills: draft.relevantSkills,
      forbiddenSkills,
      requiredTools: names,
      forbiddenTools: TOOL_NAMES.filter((name) => !required.has(name)),
      expectedState: {
        fixtureId: world.fixtureId,
        worldHash: worldHash(world),
        toolEvidence,
        requiredEffects: names.filter((name) => MUTATING_TOOLS.includes(name)),
      },
      expectedDelivery: { document, fields },
      requiresRevision: draft.requiresRevision,
    },
  };
  const parsed = CaseV1.safeParse({ ...body, contentHash: artifactHash(body) });
  return parsed.success
    ? { case: parsed.data, issues }
    : {
        issues: parsed.error.issues.map((issue) => ({
          code: 'compiled_schema',
          caseId: draft.id,
          detail: `${issue.path.join('.')}: ${issue.message}`,
        })),
      };
};

/** Compiles strict authored drafts without a model or external side effect. */
export const compileCaseDrafts = (input: unknown): CaseCompilationReport => {
  const invalid = schemaIssues(input);
  const parsed = CaseDraftDocumentV1.safeParse(input);
  if (!parsed.success) {
    return {
      schemaVersion: 1,
      valid: false,
      cases: [],
      issues: invalid,
      humanAuditRequired,
    };
  }
  const compiled = parsed.data.cases.map(compileOne);
  const issues = compiled.flatMap((entry) => entry.issues);
  const cases = compiled.flatMap((entry) =>
    entry.case === undefined ? [] : [entry.case],
  );
  return {
    schemaVersion: 1,
    valid: issues.length === 0,
    cases,
    issues,
    humanAuditRequired,
  };
};

/** Compiles and aggregates every deterministic calibration-corpus issue. */
export const compileCalibrationCases = (
  input: unknown,
  pilot: readonly Case[],
  confirmatory: readonly Case[] = [],
): CaseCompilationReport => {
  const compiled = compileCaseDrafts(input);
  const issues = [
    ...compiled.issues,
    ...validateCalibrationCases(compiled.cases, pilot, confirmatory),
  ];
  return { ...compiled, valid: issues.length === 0, issues };
};
