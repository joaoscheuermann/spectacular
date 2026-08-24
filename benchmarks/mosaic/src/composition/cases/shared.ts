import {
  PlanningCaseSchema,
  type CompositionClass,
  type PlanningCase,
  type PlanningDomain,
  type PlanningPhaseCriteria,
} from '../planning-schema.js';

export interface BehaviorFixture {
  readonly id: string;
  readonly description: string;
  readonly source: 'request' | 'catalog';
}

export interface SkillFixture {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly body: string;
  readonly behaviorIds: readonly string[];
  readonly toolIds: readonly string[];
}

export interface DomainFixture {
  readonly domain: PlanningDomain;
  readonly behaviors: readonly BehaviorFixture[];
  readonly skills: readonly SkillFixture[];
}

export interface CaseSpec {
  readonly id: string;
  readonly title: string;
  readonly compositionClass: CompositionClass;
  readonly request: string;
  readonly requestBehaviors: readonly BehaviorFixture[];
  readonly outputs: readonly {
    readonly id: string;
    readonly description: string;
  }[];
  readonly roles: readonly {
    readonly id: string;
    readonly description: string;
    readonly outputIds: readonly string[];
    readonly behaviorIds: readonly string[];
  }[];
  readonly dependencies: readonly {
    readonly id: string;
    readonly beforeRoleId: string;
    readonly afterRoleId: string;
    readonly allowSameNode: boolean;
  }[];
  readonly relevantSkillIds: readonly string[];
  readonly baseToolIds: readonly string[];
  readonly declaredToolIds: readonly string[];
  readonly p0: PlanningPhaseCriteria;
  readonly p1: PlanningPhaseCriteria;
}

/** Materializes and validates one self-contained controlled planning case. */
export const buildPlanningCase = (
  fixture: DomainFixture,
  spec: CaseSpec,
): PlanningCase => {
  const relevant = new Set(spec.relevantSkillIds);

  return PlanningCaseSchema.parse({
    schemaVersion: 1,
    id: spec.id,
    title: spec.title,
    domain: fixture.domain,
    compositionClass: spec.compositionClass,
    request: spec.request,
    catalog: fixture.skills.map((skill) => ({
      ...skill,
      behaviorIds: [...skill.behaviorIds],
      toolIds: [...skill.toolIds],
      relevance: relevant.has(skill.id) ? 'relevant' : 'distractor',
    })),
    tools: {
      base: [...spec.baseToolIds],
      declared: [...spec.declaredToolIds],
    },
    gold: {
      outputs: spec.outputs.map((output) => ({ ...output })),
      behaviors: [...spec.requestBehaviors, ...fixture.behaviors].map(
        (behavior) => ({ ...behavior }),
      ),
      roles: spec.roles.map((role) => ({
        ...role,
        outputIds: [...role.outputIds],
        behaviorIds: [...role.behaviorIds],
      })),
      dependencies: spec.dependencies.map((dependency) => ({ ...dependency })),
      relevantSkillIds: [...spec.relevantSkillIds],
      distractorSkillIds: fixture.skills
        .filter(({ id }) => !relevant.has(id))
        .map(({ id }) => id),
    },
    criteria: { p0: spec.p0, p1: spec.p1 },
  });
};

export const requestBehavior = (
  id: string,
  description: string,
): BehaviorFixture => ({ id, description, source: 'request' });

export const catalogBehavior = (
  id: string,
  description: string,
): BehaviorFixture => ({ id, description, source: 'catalog' });

export const dependency = (
  id: string,
  beforeRoleId: string,
  afterRoleId: string,
  allowSameNode = false,
) => ({ id, beforeRoleId, afterRoleId, allowSameNode });

export const phase = (
  roles: PlanningPhaseCriteria['roles'],
  dependencyIds: readonly string[],
  nodeCount: PlanningPhaseCriteria['nodeCount'],
  forbiddenBehaviorIds: readonly string[] = [],
): PlanningPhaseCriteria => ({
  roles: roles.map((role) => ({
    ...role,
    outputIds: [...role.outputIds],
    behaviorIds: [...role.behaviorIds],
  })),
  dependencyIds: [...dependencyIds],
  forbiddenBehaviorIds: [...forbiddenBehaviorIds],
  nodeCount: { ...nodeCount },
});

export const criterion = (
  roleId: string,
  outputIds: readonly string[],
  behaviorIds: readonly string[],
) => ({ roleId, outputIds: [...outputIds], behaviorIds: [...behaviorIds] });
