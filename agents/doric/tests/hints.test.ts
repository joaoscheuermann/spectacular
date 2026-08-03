import assert from 'node:assert/strict';
import test from 'node:test';

import hints from '../src/lib/decomposition/hints/index.js';
import type { Graph } from '../src/lib/types/graph.js';
import type { Skill } from '../src/lib/types/skill.js';

test('extracts hints from all candidate skills in parallel', async () => {
  const graph: Graph = {
    revision: 'P0',
    nodes: [
      {
        id: 'release-ready',
        goal: 'The release is ready.',
        doneWhen: ['Release checks pass.'],
        dependsOn: [],
        status: 'pending',
        deliver: true,
        index: 0,
      },
    ],
  };
  const skills: Array<Skill> = [
    {
      name: 'release-checks',
      description: 'Checks a release.',
      body: 'Confirm every required release check.',
      allowedTools: [],
    },
    {
      name: 'release-notes',
      description: 'Documents a release.',
      body: 'Confirm the release notes are complete.',
      allowedTools: [],
    },
  ];
  let started = 0;
  let release: () => void = () => undefined;
  const allStarted = new Promise<void>((resolve) => {
    release = resolve;
  });
  const provider = {
    complete: async () => {
      started += 1;

      if (started === skills.length) release();

      await allStarted;

      return {
        structured: {
          hints: [
            {
              effect: 'gap' as const,
              evidence: 'The skill identifies a required release result.',
            },
          ],
        },
      };
    },
  };

  const result = await hints(
    'model',
    graph,
    [{ goal: 'release-ready', skills }],
    {
      provider: provider as never,
      logger: { info: () => undefined } as never,
      vectors: {} as never,
    },
  );

  assert.equal(started, 2);
  assert.deepEqual(
    [...result].map((extraction) => extraction.skill.name),
    ['release-checks', 'release-notes'],
  );
});
