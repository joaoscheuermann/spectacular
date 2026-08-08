import { z } from 'zod';

import {
  CompositionClass,
  Domain,
  Hash,
  Id,
  JsonValue,
  SchemaVersion,
  StudyPhase,
} from './common.js';

export const CriterionV1 = z
  .object({
    id: Id,
    description: z.string().trim().min(1),
  })
  .strict();

export const ToolEvidenceV1 = z
  .object({
    name: Id,
    input: JsonValue,
    evidenceHash: Hash,
  })
  .strict();

export const ExpectedStateV1 = z
  .object({
    fixtureId: Id,
    worldHash: Hash,
    toolEvidence: z.array(ToolEvidenceV1),
    requiredEffects: z.array(Id),
  })
  .strict();

export const ExpectedDeliveryV1 = z
  .object({
    contains: z.array(z.string().min(1)).min(1),
  })
  .strict();

export const CaseGoldV1 = z
  .object({
    criteria: z.array(CriterionV1).min(1),
    requiredSkills: z.array(Id),
    relevantSkills: z.array(Id),
    forbiddenSkills: z.array(Id),
    requiredTools: z.array(Id),
    forbiddenTools: z.array(Id),
    expectedState: ExpectedStateV1,
    expectedDelivery: ExpectedDeliveryV1,
    requiresRevision: z.boolean(),
  })
  .strict();

export const CompositionSignatureV1 = z
  .object({
    skillCount: z.union([z.literal(0), z.literal(1), z.literal('many')]),
    toolRequirement: z.enum(['none', 'base', 'declared', 'mixed']),
    toolCount: z.union([z.literal(0), z.literal(1), z.literal('many')]),
    requiresRevision: z.boolean(),
    requiresExternalEffect: z.boolean(),
  })
  .strict();

/** A frozen, self-contained benchmark task and its deterministic oracle. */
export const CaseV1 = z
  .object({
    schemaVersion: SchemaVersion,
    id: Id,
    familyId: Id,
    phase: StudyPhase,
    title: z.string().trim().min(1),
    domain: Domain,
    compositionClass: CompositionClass,
    focusGoalRole: Id,
    composition: CompositionSignatureV1,
    adaptive: z.boolean(),
    request: z.string().trim().min(1),
    fixtureIds: z.array(Id),
    tags: z.array(Id),
    gold: CaseGoldV1,
    contentHash: Hash,
  })
  .strict();

export type Case = z.infer<typeof CaseV1>;
