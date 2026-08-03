import { VectorDatabase } from 'victor';

import { Graph, Node } from '../../types/graph.js';
import { markdownNumberedList } from '../utils/index.js';
import { Skill } from '../../types/skill.js';

export interface SkillsContext {
  vectors: VectorDatabase;
}

export interface GoalSkills {
  goal: string;
  skills: Array<Skill>;
}

// TODO: adicionar contexto adicional, como resultados dos nodes anteriores :D
const search = (prompt: string, node: Node) => `
Original Request:
${prompt}

Current Goal:
${node.goal}

Completion Criteria:
${markdownNumberedList(node.doneWhen)}

Relevant Context:
None.
`;

export default async function candidates(prompt: string, graph: Graph, topK: number, { vectors }: SkillsContext) {
  const results: Array<GoalSkills> = [];

  for (const node of graph.nodes) {
    const skills = (await vectors.search(search(prompt, node), topK)).map(result => result.data as Skill);

    results.push({ goal: node.id, skills })
  }

  return results
}
