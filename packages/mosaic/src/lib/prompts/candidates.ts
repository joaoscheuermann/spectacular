import type { Node } from '../types/graph.js';
import { markdownNumberedList } from './list.js';

// TODO: adicionar contexto adicional, como resultados dos nodes anteriores :D
export function search(prompt: string, node: Node) {
  return `
Original Request:
${prompt}

Current Goal:
${node.goal}

Completion Criteria:
${markdownNumberedList(node.doneWhen)}

Relevant Context:
None.
`;
}
