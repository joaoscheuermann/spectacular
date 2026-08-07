export interface GraphNodeShape {
  readonly id: string;
  readonly dependsOn: readonly string[];
  readonly deliver: boolean;
}

export interface ValidationResult {
  readonly success: boolean;
  readonly errors: readonly string[];
}

/** Validates the cross-node invariants shared by planned and runtime graphs. */
export function validateGraphSchema(plan: {
  readonly nodes: readonly GraphNodeShape[];
}): ValidationResult {
  const errors: string[] = [];
  const nodeIds = new Set<string>();

  for (const node of plan.nodes) {
    if (nodeIds.has(node.id)) {
      errors.push(`Duplicate node ID found: "${node.id}".`);
    }
    nodeIds.add(node.id);
  }

  for (const node of plan.nodes) {
    for (const dependency of node.dependsOn) {
      if (!nodeIds.has(dependency)) {
        errors.push(
          `Node "${node.id}" references a non-existent dependency: "${dependency}".`,
        );
      }
    }
  }

  const dependents = new Set(plan.nodes.flatMap(({ dependsOn }) => dependsOn));
  const invalidDelivery = plan.nodes.find(
    ({ id, deliver }) => deliver && dependents.has(id),
  );
  if (invalidDelivery !== undefined) {
    errors.push(`Deliverable node "${invalidDelivery.id}" must be terminal.`);
  }

  const terminalDeliverable = plan.nodes.some(
    ({ id, deliver }) => deliver && !dependents.has(id),
  );
  if (!terminalDeliverable) {
    errors.push('At least one terminal node must be marked for delivery.');
  }

  const dependencies = new Map(
    plan.nodes.map((node) => [node.id, node.dependsOn] as const),
  );
  const visiting = new Set<string>();
  const visited = new Set<string>();

  const visit = (id: string): boolean => {
    if (visiting.has(id)) return true;
    if (visited.has(id)) return false;

    visiting.add(id);
    const cyclic = (dependencies.get(id) ?? []).some(
      (dependency) => dependencies.has(dependency) && visit(dependency),
    );
    visiting.delete(id);
    visited.add(id);
    return cyclic;
  };

  if ([...nodeIds].some(visit)) {
    errors.push('Circular dependency detected in graph.');
  }

  return { success: errors.length === 0, errors };
}
