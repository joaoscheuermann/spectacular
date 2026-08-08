import type { ToolName } from '../config/index.js';

export type CatalogDomain =
  | 'documents-finance'
  | 'software'
  | 'artifacts'
  | 'communication';

export type RelationKind = 'equivalent' | 'overlap' | 'distractor' | 'conflict';

export interface SkillRelation {
  readonly kind: RelationKind;
  readonly skillId: string;
  readonly rationale: string;
}

export interface MicroSkill {
  readonly id: string;
  readonly domain: CatalogDomain;
  readonly title: string;
  readonly description: string;
  readonly body: string;
  readonly allowedTools: readonly ToolName[];
  readonly relations: readonly SkillRelation[];
}

export interface SkillSeed {
  readonly slug: string;
  readonly title: string;
  readonly description: string;
  readonly procedure: readonly string[];
  readonly tools: readonly ToolName[];
  readonly relation: RelationKind;
  readonly peer: string;
  readonly rationale: string;
}
