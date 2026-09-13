/* Retrieve additional skill guidance for a concrete need during direct execution. */
import { z } from 'zod';

import { defineTool } from 'tool';

import { selectSkills } from './gate.mjs';

const text = z.string().trim().min(1);

export const createSkillSearch = ({ runtime, search, request, searches }) =>
  defineTool({
    name: 'search_skills',
    description:
      'Find skills for a current need: query describes the need; context gives relevant observations. Returns complete skill bodies or an empty list. Provides guidance only, without executing it or adding tools.',
    input: z.object({ query: text, context: text }).strict(),
    output: z
      .object({
        skills: z.array(z.object({ name: text, body: text }).strict()),
      })
      .strict(),
    execute: async (_sandbox, input) => {
      const id = searches.length + 1;
      const entry = { id, ...input, skills: [] };

      searches.push(entry);

      await runtime.record('search-skills.' + id + '.input', input);

      const need = input.query + '\n\nObserved context:\n' + input.context;
      const candidates = await search(request + '\n\nCurrent need:\n' + need);

      await runtime.record('retrieval.search-skills.' + id, candidates);

      const selected = await selectSkills({
        runtime,
        request,
        goals: [need],
        candidates,
        stage: 'gate.search-skills.' + id,
      });
      const skills = selected.map(({ name, body }) => ({ name, body }));

      entry.skills = skills.map(({ name }) => name);

      return runtime.record('search-skills.' + id, { skills });
    },
  });
