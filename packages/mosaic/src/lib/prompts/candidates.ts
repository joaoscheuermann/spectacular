import type { Node } from '../types/graph.js';
import { fenced, section } from './context.js';

export const search = (request: string, node: Node): string =>
  [
    '# Skill Candidate Search',
    section('Original Request', request),
    section('Current Goal', node.goal),
    '## Completion Criteria',
    ...node.doneWhen.flatMap((criterion, index) => [
      `### Criterion ${index}`,
      fenced(criterion),
    ]),
  ].join('\n\n');
