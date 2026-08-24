import { z } from 'zod';

export const PlanningDomainSchema = z.enum([
  'documents-finance',
  'software',
  'artifacts',
  'communications',
]);

export const CompositionClassSchema = z.enum(['A', 'B', 'C', 'D', 'E', 'F']);

const IdSchema = z
  .string()
  .trim()
  .min(1)
  .regex(/^[a-z][a-z0-9]*(?:[.:-][a-z0-9]+)*$/u);
const TextSchema = z.string().trim().min(1);

const unique = (
  values: readonly string[],
  message: string,
): string | undefined =>
  new Set(values).size === values.length ? undefined : message;

const IdsSchema = z.array(IdSchema).superRefine((values, context) => {
  const message = unique(values, 'Identifiers must be unique.');
  if (message !== undefined) context.addIssue({ code: 'custom', message });
});

const NodeIdsSchema = z.array(TextSchema).superRefine((values, context) => {
  const message = unique(values, 'Node identifiers must be unique.');
  if (message !== undefined) context.addIssue({ code: 'custom', message });
});

export const PlanningOutputSchema = z
  .object({ id: IdSchema, description: TextSchema })
  .strict();

export const PlanningBehaviorSchema = z
  .object({
    id: IdSchema,
    description: TextSchema,
    source: z.enum(['request', 'catalog']),
  })
  .strict();

export const PlanningSkillSchema = z
  .object({
    id: IdSchema,
    name: IdSchema,
    description: TextSchema,
    body: TextSchema,
    behaviorIds: IdsSchema.min(1),
    toolIds: IdsSchema,
    relevance: z.enum(['relevant', 'distractor']),
  })
  .strict();

export const PlanningRoleSchema = z
  .object({
    id: IdSchema,
    description: TextSchema,
    outputIds: IdsSchema.min(1),
    behaviorIds: IdsSchema.min(1),
  })
  .strict();

export const PlanningDependencySchema = z
  .object({
    id: IdSchema,
    beforeRoleId: IdSchema,
    afterRoleId: IdSchema,
    allowSameNode: z.boolean(),
  })
  .strict();

export const PlanningRoleCriterionSchema = z
  .object({
    roleId: IdSchema,
    outputIds: IdsSchema,
    behaviorIds: IdsSchema,
  })
  .strict();

export const PlanningPhaseCriteriaSchema = z
  .object({
    roles: z
      .array(PlanningRoleCriterionSchema)
      .min(1)
      .superRefine((roles, context) => {
        const message = unique(
          roles.map(({ roleId }) => roleId),
          'Criterion role identifiers must be unique.',
        );
        if (message !== undefined)
          context.addIssue({ code: 'custom', message });
      }),
    dependencyIds: IdsSchema,
    forbiddenBehaviorIds: IdsSchema,
    nodeCount: z
      .object({
        min: z.number().int().safe().positive(),
        max: z.number().int().safe().positive(),
      })
      .strict()
      .refine(({ min, max }) => min <= max, {
        message: 'Minimum node count must not exceed maximum node count.',
      }),
  })
  .strict();

export const PlanningCaseSchema = z
  .object({
    schemaVersion: z.literal(1),
    id: IdSchema,
    title: TextSchema,
    domain: PlanningDomainSchema,
    compositionClass: CompositionClassSchema,
    request: TextSchema,
    catalog: z.array(PlanningSkillSchema).min(3),
    tools: z.object({ base: IdsSchema, declared: IdsSchema }).strict(),
    gold: z
      .object({
        outputs: z.array(PlanningOutputSchema).min(1),
        behaviors: z.array(PlanningBehaviorSchema).min(1),
        roles: z.array(PlanningRoleSchema).min(1),
        dependencies: z.array(PlanningDependencySchema),
        relevantSkillIds: IdsSchema,
        distractorSkillIds: IdsSchema.min(2),
      })
      .strict(),
    criteria: z
      .object({
        p0: PlanningPhaseCriteriaSchema,
        p1: PlanningPhaseCriteriaSchema,
      })
      .strict(),
  })
  .strict()
  .superRefine((value, context) => validateCase(value, context));

export const PlanningObservationNodeSchema = z
  .object({
    id: TextSchema,
    roleIds: IdsSchema.min(1),
    outputIds: IdsSchema,
    behaviorIds: IdsSchema,
    dependsOn: NodeIdsSchema,
  })
  .strict();

export const PlanningObservationSchema = z
  .object({ nodes: z.array(PlanningObservationNodeSchema).min(1) })
  .strict()
  .superRefine((value, context) => {
    const ids = value.nodes.map(({ id }) => id);
    addUniqueIssue(
      ids,
      context,
      ['nodes'],
      'Observation node IDs must be unique.',
    );
    const known = new Set(ids);
    value.nodes.forEach((node, index) => {
      node.dependsOn.forEach((dependency, dependencyIndex) => {
        if (known.has(dependency) && dependency !== node.id) return;
        context.addIssue({
          code: 'custom',
          path: ['nodes', index, 'dependsOn', dependencyIndex],
          message: 'Observation dependencies must reference another node.',
        });
      });
    });
    if (hasCycle(value.nodes)) {
      context.addIssue({
        code: 'custom',
        path: ['nodes'],
        message: 'Observation dependencies must be acyclic.',
      });
    }
  });

export const PlanningGraphNodeSchema = z
  .object({
    id: TextSchema,
    goal: TextSchema,
    doneWhen: z.array(TextSchema).min(1),
    dependsOn: z.array(TextSchema),
    deliver: z.boolean(),
  })
  .strict();

export const PlanningGraphSchema = z
  .object({ nodes: z.array(PlanningGraphNodeSchema).min(1) })
  .strict()
  .superRefine((value, context) => {
    const ids = value.nodes.map(({ id }) => id);
    addUniqueIssue(ids, context, ['nodes'], 'Plan node IDs must be unique.');
    const known = new Set(ids);
    value.nodes.forEach((node, index) => {
      const duplicate = unique(
        node.dependsOn,
        'Plan dependencies must be unique.',
      );
      if (duplicate !== undefined) {
        context.addIssue({
          code: 'custom',
          path: ['nodes', index, 'dependsOn'],
          message: duplicate,
        });
      }
      node.dependsOn.forEach((dependency, dependencyIndex) => {
        if (known.has(dependency) && dependency !== node.id) return;
        context.addIssue({
          code: 'custom',
          path: ['nodes', index, 'dependsOn', dependencyIndex],
          message: 'Plan dependencies must reference another node.',
        });
      });
    });
    if (hasCycle(value.nodes)) {
      context.addIssue({
        code: 'custom',
        path: ['nodes'],
        message: 'Plan dependencies must be acyclic.',
      });
    }
    if (!value.nodes.some(({ deliver }) => deliver)) {
      context.addIssue({
        code: 'custom',
        path: ['nodes'],
        message: 'At least one terminal node must be deliverable.',
      });
    }
    const dependedOn = new Set(
      value.nodes.flatMap(({ dependsOn }) => dependsOn),
    );
    value.nodes.forEach((node, index) => {
      if (!node.deliver || !dependedOn.has(node.id)) return;
      context.addIssue({
        code: 'custom',
        path: ['nodes', index, 'deliver'],
        message: 'A deliverable plan node must be terminal.',
      });
    });
  });

export type PlanningCase = z.output<typeof PlanningCaseSchema>;
export type PlanningDomain = z.output<typeof PlanningDomainSchema>;
export type CompositionClass = z.output<typeof CompositionClassSchema>;
export type PlanningPhaseCriteria = z.output<
  typeof PlanningPhaseCriteriaSchema
>;
export type PlanningObservation = z.output<typeof PlanningObservationSchema>;
export type PlanningGraph = z.output<typeof PlanningGraphSchema>;

type CaseInput = z.input<typeof PlanningCaseSchema>;
type Refinement = z.RefinementCtx;

const validateCase = (value: CaseInput, context: Refinement): void => {
  addUniqueIssue(
    value.catalog.map(({ id }) => id),
    context,
    ['catalog'],
    'Catalog skill IDs must be unique.',
  );
  addUniqueIssue(
    value.catalog.map(({ name }) => name),
    context,
    ['catalog'],
    'Catalog skill names must be unique.',
  );
  addUniqueIssue(
    value.gold.outputs.map(({ id }) => id),
    context,
    ['gold', 'outputs'],
    'Output IDs must be unique.',
  );
  addUniqueIssue(
    value.gold.behaviors.map(({ id }) => id),
    context,
    ['gold', 'behaviors'],
    'Behavior IDs must be unique.',
  );
  addUniqueIssue(
    value.gold.roles.map(({ id }) => id),
    context,
    ['gold', 'roles'],
    'Role IDs must be unique.',
  );
  addUniqueIssue(
    value.gold.dependencies.map(({ id }) => id),
    context,
    ['gold', 'dependencies'],
    'Dependency IDs must be unique.',
  );

  const outputs = new Set(value.gold.outputs.map(({ id }) => id));
  const behaviors = new Set(value.gold.behaviors.map(({ id }) => id));
  const roles = new Map(value.gold.roles.map((role) => [role.id, role]));
  const dependencies = new Map(
    value.gold.dependencies.map((dependency) => [dependency.id, dependency]),
  );
  const skills = new Set(value.catalog.map(({ id }) => id));

  value.catalog.forEach((skill, index) =>
    validateReferences(
      skill.behaviorIds,
      behaviors,
      context,
      ['catalog', index, 'behaviorIds'],
      'Catalog skills must reference known behaviors.',
    ),
  );
  value.gold.roles.forEach((role, index) => {
    validateReferences(
      role.outputIds,
      outputs,
      context,
      ['gold', 'roles', index, 'outputIds'],
      'Roles must reference known outputs.',
    );
    validateReferences(
      role.behaviorIds,
      behaviors,
      context,
      ['gold', 'roles', index, 'behaviorIds'],
      'Roles must reference known behaviors.',
    );
  });
  value.gold.dependencies.forEach((dependency, index) => {
    if (
      roles.has(dependency.beforeRoleId) &&
      roles.has(dependency.afterRoleId) &&
      dependency.beforeRoleId !== dependency.afterRoleId
    )
      return;
    context.addIssue({
      code: 'custom',
      path: ['gold', 'dependencies', index],
      message: 'Dependencies must connect two known, distinct roles.',
    });
  });

  validateReferences(
    value.gold.relevantSkillIds,
    skills,
    context,
    ['gold', 'relevantSkillIds'],
    'Relevant skill IDs must exist in the catalog.',
  );
  validateReferences(
    value.gold.distractorSkillIds,
    skills,
    context,
    ['gold', 'distractorSkillIds'],
    'Distractor skill IDs must exist in the catalog.',
  );
  const classified = new Set([
    ...value.gold.relevantSkillIds,
    ...value.gold.distractorSkillIds,
  ]);
  if (
    classified.size !== value.catalog.length ||
    value.catalog.some(
      ({ id, relevance }) =>
        !classified.has(id) ||
        (relevance === 'relevant') !== value.gold.relevantSkillIds.includes(id),
    )
  ) {
    context.addIssue({
      code: 'custom',
      path: ['gold'],
      message:
        'Gold skill relevance must classify every catalog entry exactly once.',
    });
  }

  validatePhase(
    value.criteria.p0,
    'p0',
    roles,
    dependencies,
    behaviors,
    context,
  );
  validatePhase(
    value.criteria.p1,
    'p1',
    roles,
    dependencies,
    behaviors,
    context,
  );

  const p0Atoms = phaseAtoms(value.criteria.p0);
  const p1Atoms = phaseAtoms(value.criteria.p1);
  const missing = [...p0Atoms].filter((atom) => !p1Atoms.has(atom));
  if (missing.length > 0) {
    context.addIssue({
      code: 'custom',
      path: ['criteria', 'p1'],
      message: 'P1 criteria must preserve every semantic requirement from P0.',
    });
  }

  validateComposition(value, context);

  if (
    (value.compositionClass === 'A' || value.compositionClass === 'B') &&
    (p0Atoms.size !== p1Atoms.size ||
      [...p0Atoms].some((atom) => !p1Atoms.has(atom)))
  ) {
    context.addIssue({
      code: 'custom',
      path: ['criteria'],
      message: 'Zero-skill classes must use identical P0 and P1 semantics.',
    });
  }

  if (
    ['C', 'D', 'E', 'F'].includes(value.compositionClass) &&
    !hasCatalogGain(value, p0Atoms, p1Atoms)
  ) {
    context.addIssue({
      code: 'custom',
      path: ['criteria', 'p1'],
      message: 'Skill-bearing classes must add a catalog-sourced P1 behavior.',
    });
  }
};

const validatePhase = (
  phase: z.input<typeof PlanningPhaseCriteriaSchema>,
  name: 'p0' | 'p1',
  roles: ReadonlyMap<string, z.input<typeof PlanningRoleSchema>>,
  dependencies: ReadonlyMap<string, z.input<typeof PlanningDependencySchema>>,
  behaviors: ReadonlySet<string>,
  context: Refinement,
): void => {
  phase.roles.forEach((criterion, index) => {
    const role = roles.get(criterion.roleId);
    if (role === undefined) {
      context.addIssue({
        code: 'custom',
        path: ['criteria', name, 'roles', index, 'roleId'],
        message: 'Phase criteria must reference a known role.',
      });
      return;
    }
    validateReferences(
      criterion.outputIds,
      new Set(role.outputIds),
      context,
      ['criteria', name, 'roles', index, 'outputIds'],
      'Phase outputs must belong to their referenced role.',
    );
    validateReferences(
      criterion.behaviorIds,
      new Set(role.behaviorIds),
      context,
      ['criteria', name, 'roles', index, 'behaviorIds'],
      'Phase behaviors must belong to their referenced role.',
    );
  });
  validateReferences(
    phase.dependencyIds,
    new Set(dependencies.keys()),
    context,
    ['criteria', name, 'dependencyIds'],
    'Phase criteria must reference known dependencies.',
  );
  validateReferences(
    phase.forbiddenBehaviorIds,
    behaviors,
    context,
    ['criteria', name, 'forbiddenBehaviorIds'],
    'Forbidden behavior IDs must be known.',
  );
  const required = new Set(
    phase.roles.flatMap(({ behaviorIds }) => behaviorIds),
  );
  if (phase.forbiddenBehaviorIds.some((id) => required.has(id))) {
    context.addIssue({
      code: 'custom',
      path: ['criteria', name, 'forbiddenBehaviorIds'],
      message: 'A phase cannot both require and forbid the same behavior.',
    });
  }
};

const validateComposition = (value: CaseInput, context: Refinement): void => {
  const relevant = value.gold.relevantSkillIds.length;
  const toolCount = new Set([...value.tools.base, ...value.tools.declared])
    .size;
  const valid =
    (value.compositionClass === 'A' && relevant === 0 && toolCount === 0) ||
    (value.compositionClass === 'B' &&
      relevant === 0 &&
      value.tools.base.length > 0 &&
      value.tools.declared.length === 0) ||
    (value.compositionClass === 'C' && relevant >= 1 && toolCount === 0) ||
    (value.compositionClass === 'D' &&
      relevant === 1 &&
      value.tools.declared.length > 0) ||
    (value.compositionClass === 'E' && relevant >= 2 && toolCount <= 1) ||
    (value.compositionClass === 'F' && relevant >= 2 && toolCount >= 2);
  if (valid) return;
  context.addIssue({
    code: 'custom',
    path: ['compositionClass'],
    message: 'Catalog and tool cardinality must match the composition class.',
  });
};

const hasCatalogGain = (
  value: CaseInput,
  p0Atoms: ReadonlySet<string>,
  p1Atoms: ReadonlySet<string>,
): boolean => {
  const catalog = new Set(
    value.gold.behaviors
      .filter(({ source }) => source === 'catalog')
      .map(({ id }) => id),
  );
  return [...p1Atoms].some((atom) => {
    if (p0Atoms.has(atom) || !atom.startsWith('behavior:')) return false;
    return catalog.has(atom.slice(atom.lastIndexOf(':') + 1));
  });
};

export const planningPhaseAtoms = (
  phase: PlanningPhaseCriteria,
): ReadonlySet<string> => phaseAtoms(phase);

const phaseAtoms = (
  phase: z.input<typeof PlanningPhaseCriteriaSchema>,
): Set<string> =>
  new Set([
    ...phase.roles.flatMap(({ roleId, outputIds, behaviorIds }) => [
      `role:${roleId}`,
      ...outputIds.map((id) => `output:${roleId}:${id}`),
      ...behaviorIds.map((id) => `behavior:${roleId}:${id}`),
    ]),
    ...phase.dependencyIds.map((id) => `dependency:${id}`),
    ...phase.forbiddenBehaviorIds.map((id) => `forbidden:${id}`),
    'node-count',
  ]);

const addUniqueIssue = (
  values: readonly string[],
  context: Refinement,
  path: readonly PropertyKey[],
  message: string,
): void => {
  if (new Set(values).size === values.length) return;
  context.addIssue({ code: 'custom', path: [...path], message });
};

const validateReferences = (
  values: readonly string[],
  known: ReadonlySet<string>,
  context: Refinement,
  path: readonly PropertyKey[],
  message: string,
): void => {
  values.forEach((value, index) => {
    if (known.has(value)) return;
    context.addIssue({ code: 'custom', path: [...path, index], message });
  });
};

const hasCycle = (
  nodes: readonly {
    readonly id: string;
    readonly dependsOn: readonly string[];
  }[],
): boolean => {
  const dependencies = new Map(nodes.map((node) => [node.id, node.dependsOn]));
  const visiting = new Set<string>();
  const visited = new Set<string>();
  const visit = (id: string): boolean => {
    if (visiting.has(id)) return true;
    if (visited.has(id)) return false;
    visiting.add(id);
    const cycle = (dependencies.get(id) ?? []).some(visit);
    visiting.delete(id);
    visited.add(id);
    return cycle;
  };
  return nodes.some(({ id }) => visit(id));
};
