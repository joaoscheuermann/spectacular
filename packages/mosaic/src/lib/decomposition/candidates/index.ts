import { VectorDatabase } from 'victor';

import { Graph } from '../../types/graph.js';
import { search } from '../../prompts/candidates/index.js';
import type { Skill } from 'bundle';

export interface SkillsContext {
  vectors: VectorDatabase;
}

export interface GoalSkills {
  goal: string;
  skills: Array<Skill>;
}

export default async function candidates(
  prompt: string,
  graph: Graph,
  topK: number,
  { vectors }: SkillsContext,
) {
  const results: Array<GoalSkills> = [];

  for (const node of graph.nodes) {
    const skills = (await vectors.search(search(prompt, node), topK)).map(
      (result) => result.data as Skill,
    );

    results.push({ goal: node.id, skills });
  }

  return results;
}
