import { z } from 'zod';
import { GraphSchema } from './index.js'; // Assuming your schemas are here

export interface ValidationResult {
  success: boolean;
  errors: string[];
}

/**
 * Validates a parsed MOSAIC plan against the normative runtime invariants
 * defined in Appendix A.1 of the MOSAIC Core Profile 0.1 specification.
 */
export function validateGraphSchema(plan: z.infer<typeof GraphSchema>): ValidationResult {
  const errors: string[] = [];
  const nodeIds = new Set<string>();

  // 1. Check for unique objective IDs (MOSAIC A.1.3)
  for (const node of plan.nodes) {
    if (nodeIds.has(node.id)) {
      errors.push(`Duplicate node ID found: "${node.id}". Node IDs must be globally unique within a plan.`);
    }
    nodeIds.add(node.id);
  }

  // 2. Verify existence of all declared dependencies (MOSAIC A.1.4)
  for (const node of plan.nodes) {
    for (const depId of node.dependsOn) {
      if (!nodeIds.has(depId)) {
        errors.push(`Node "${node.id}" references a non-existent dependency: "${depId}".`);
      }
    }
  }

  // 3. Ensure there is at least one terminal deliverable node (MOSAIC A.1.8)
  const hasDeliverNode = plan.nodes.some((node) => node.deliver);
  if (!hasDeliverNode) {
    errors.push("The plan is invalid: At least one terminal goal node must be marked for delivery (deliver: true).");
  }

  // 4. Cycle Detection - Verify the graph is strictly acyclic (MOSAIC A.1.5)
  const graphMap = new Map<string, string[]>();
  for (const node of plan.nodes) {
    graphMap.set(node.id, node.dependsOn);
  }

  const visiting = new Set<string>();
  const visited = new Set<string>();
  const cyclePath: string[] = [];

  function dfs(nodeId: string): boolean {
    visiting.add(nodeId);
    cyclePath.push(nodeId);

    const dependencies = graphMap.get(nodeId) || [];
    for (const depId of dependencies) {
      // If we encounter a node currently in the recursion stack, a cycle exists
      if (visiting.has(depId)) {
        const cycleStartIndex = cyclePath.indexOf(depId);
        const cycleString = [...cyclePath.slice(cycleStartIndex), depId].join(" ➔ ");
        errors.push(`Circular dependency detected in graph: ${cycleString}`);
        return false;
      }

      if (!visited.has(depId)) {
        if (!dfs(depId)) return false;
      }
    }

    visiting.delete(nodeId);
    cyclePath.pop();
    visited.add(nodeId);
    return true;
  }

  // Traverse every unvisited node to check all disconnected subgraphs
  for (const nodeId of nodeIds) {
    if (!visited.has(nodeId)) {
      dfs(nodeId);
    }
  }

  return {
    success: errors.length === 0,
    errors,
  };
}
