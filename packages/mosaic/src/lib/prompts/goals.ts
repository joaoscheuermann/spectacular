import { section } from './context.js';

export const system = (): string =>
  [
    'Create the smallest outcome-oriented initial plan P0 for the request.',
    '',
    '# Planning rules',
    '',
    '- Preserve the request intent, constraints, and deliverables.',
    '- Describe observable results, not actions, tools, skills, APIs, commands,',
    '  implementation trajectories, or generic operational concerns.',
    '- Split a result only when it is independently produced or evaluated, is a',
    '  genuine prerequisite, may change later planning, or is separately requested.',
    '- Use dependencies only when one result must exist before another is produced.',
    '- Give every node a unique stable ID and at least one observable doneWhen item.',
    '- Mark only terminal user-facing results for delivery.',
    '- Keep dependencies unique, existing, and acyclic.',
    '- Do not use a skill catalog. P0 is catalog-independent.',
    '- Return only the requested structured planning output. Runtime fields are',
    '  assigned by the scheduler and must not be included.',
  ].join('\n');

export const user = (request: string): string =>
  ['# Planning Request', section('Original Request', request)].join('\n\n');
