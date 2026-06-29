import { access, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { createPromptArtifact, prompt } from '../src/index.js';
import {
  createProvider,
  createUnstructuredProvider,
  exploration,
  finishWithToolCalls,
  intent,
  messageText,
  requestUnderstanding,
  responsesWithQuestions,
  responsesWithoutQuestions,
  toolCall,
  toolText,
  workflowOptions,
} from './fake-provider.js';

test('starts prompt artifacts with only the initial user prompt in data', () => {
  const artifact = createPromptArtifact('Build the prompt workflow.');

  assert.deepEqual(artifact.data, {
    initialUserPrompt: 'Build the prompt workflow.',
  });
});

test('runs split passes and gates requirements when open questions exist', async () => {
  const root = await workspace('prompt-questions');
  const artifact = createPromptArtifact('Refactor workflow-prompt.');
  const provider = createProvider(responsesWithQuestions());

  const result = await prompt(artifact, {
    ...workflowOptions(provider.provider, root),
    maxQuestions: 1,
  });

  assert.notEqual(result, artifact);
  assert.deepEqual(artifact.data, {
    initialUserPrompt: 'Refactor workflow-prompt.',
  });
  assert.deepEqual(result.data.exploration, exploration());
  assert.equal(
    result.data.requestUnderstanding,
    'The user asked to refactor workflow-prompt into explicit passes.',
  );
  assert.deepEqual(result.data.intent, {
    goal: 'Refactor workflow-prompt.',
    scope: 'workflows/prompt',
  });
  assert.deepEqual(result.data.openQuestions, [
    {
      question: 'Which exact requirement fields should downstream agents use?',
      impact: 'Requirement extraction could choose an incompatible schema.',
      recommendation: 'Confirm the public artifact schema first.',
      options: [
        'Confirm the public artifact schema first.',
        'Use the existing requirement list fields without schema changes.',
        'Emit a separate compatibility artifact for downstream agents.',
      ],
    },
  ]);
  assert.match(messageText(provider), /at least 3 concrete solution options/u);
  assert.equal(result.data.productRequirements, undefined);
  assert.equal(result.data.technicalRequirements, undefined);
  assert.equal(provider.requests.length, 4);
  await rm(root, { recursive: true, force: true });
});

test('blocks requirement extraction when returned questions are capped to zero', async () => {
  const root = await workspace('prompt-zero-question-cap');
  const provider = createProvider(responsesWithQuestions());

  const result = await prompt(
    createPromptArtifact('Refactor workflow-prompt.'),
    {
      ...workflowOptions(provider.provider, root),
      maxQuestions: 0,
    },
  );

  assert.deepEqual(result.data.openQuestions, []);
  assert.equal(result.data.productRequirements, undefined);
  assert.equal(result.data.technicalRequirements, undefined);
  assert.equal(provider.requests.length, 4);
  await rm(root, { recursive: true, force: true });
});

test('extracts product and technical requirements only when questions are empty', async () => {
  const root = await workspace('prompt-requirements');
  const provider = createProvider(responsesWithoutQuestions());

  const result = await prompt(createPromptArtifact('Ship the workflow.'), {
    ...workflowOptions(provider.provider, root),
  });

  assert.deepEqual(result.data.openQuestions, []);
  assert.deepEqual(result.data.productRequirements, [
    'Preserve the initial request for downstream workflow steps.',
  ]);
  assert.deepEqual(result.data.technicalRequirements, [
    'Run each extraction pass with a fresh agent and message storage.',
  ]);
  assert.equal(provider.requests.length, 6);
  await rm(root, { recursive: true, force: true });
});

test('passes reasoning effort to every prompt pass provider request', async () => {
  const root = await workspace('prompt-effort');
  const provider = createProvider(responsesWithoutQuestions());

  await prompt(createPromptArtifact('Ship with high effort.'), {
    ...workflowOptions(provider.provider, root),
    effort: 'xhigh',
  });

  assert.deepEqual(
    provider.requests.map((request) => request.effort),
    ['xhigh', 'xhigh', 'xhigh', 'xhigh', 'xhigh', 'xhigh'],
  );
  await rm(root, { recursive: true, force: true });
});

test('executes exploration tools through workflow-safe wrappers', async () => {
  const root = await workspace('prompt-safe-tools');
  await write(root, 'GROUNDING.md', 'GROUNDING_SENTINEL');
  await write(root, 'AGENTS.md', 'AGENTS_SENTINEL');
  await write(root, 'README.md', 'README_SENTINEL');
  await write(root, 'docs/guide.md', 'DOCS_SENTINEL');
  await write(root, 'docs/leak.ts', 'DOCS_TS_SENTINEL');
  await write(root, '.agents/skills/example/SKILL.md', 'SKILL_SENTINEL');
  await write(root, '.agents/skills/example/leak.ts', 'AGENTS_TS_SENTINEL');
  await write(root, 'src/index.ts', 'export const marker = "SAFE_SENTINEL";');
  const provider = createProvider([
    finishWithToolCalls([
      toolCall('find', { pattern: '**/*', path: '.' }, 'call_find_root'),
      toolCall('grep', { pattern: 'SENTINEL', path: '.' }, 'call_grep_root'),
      toolCall(
        'grep',
        { pattern: 'SENTINEL', path: '.', glob: '**/*.ts' },
        'call_grep_typescript',
      ),
      toolCall('tree', { path: '.' }, 'call_tree_root'),
      toolCall('find', { pattern: '**/*', path: 'docs' }, 'call_find_docs'),
      toolCall(
        'grep',
        { pattern: 'GROUNDING_SENTINEL', path: 'GROUNDING.md' },
        'call_grep_grounding',
      ),
      toolCall('tree', { path: 'docs' }, 'call_tree_docs'),
    ]),
    exploration(),
    ...responsesWithoutQuestions().slice(1),
  ]);

  await prompt(createPromptArtifact('Inspect the repo safely.'), {
    ...workflowOptions(provider.provider, root),
  });

  const rootFind = toolText(provider, 'call_find_root');
  const rootGrep = toolText(provider, 'call_grep_root');
  const rootGrepTypescript = toolText(provider, 'call_grep_typescript');
  const rootTree = toolText(provider, 'call_tree_root');
  const docsFind = toolText(provider, 'call_find_docs');
  const groundingGrep = toolText(provider, 'call_grep_grounding');
  const docsTree = toolText(provider, 'call_tree_docs');

  assert.match(rootFind, /src\/index\.ts/);
  assert.doesNotMatch(
    rootFind,
    /GROUNDING\.md|AGENTS\.md|README\.md|docs|SKILL\.md|\.agents/,
  );
  assert.match(rootGrep, /clearly non-markdown file path/);
  assert.doesNotMatch(
    rootGrep,
    /GROUNDING_SENTINEL|AGENTS_SENTINEL|README_SENTINEL|DOCS_SENTINEL|DOCS_TS_SENTINEL|SKILL_SENTINEL|AGENTS_TS_SENTINEL/,
  );
  assert.match(rootGrepTypescript, /SAFE_SENTINEL/);
  assert.doesNotMatch(
    rootGrepTypescript,
    /DOCS_TS_SENTINEL|AGENTS_TS_SENTINEL/,
  );
  assert.match(rootTree, /src/);
  assert.doesNotMatch(
    rootTree,
    /GROUNDING\.md|AGENTS\.md|README\.md|docs|SKILL\.md|\.agents/,
  );
  assert.match(docsFind, /cannot access repository guidance markdown/);
  assert.match(groundingGrep, /cannot access repository guidance markdown/);
  assert.match(docsTree, /cannot access repository guidance markdown/);
  await rm(root, { recursive: true, force: true });
});

test('exposes find grep and tree only to the exploration provider request', async () => {
  const root = await workspace('prompt-tools');
  const provider = createProvider(responsesWithoutQuestions());

  await prompt(createPromptArtifact('Inspect the repo.'), {
    ...workflowOptions(provider.provider, root),
  });

  assert.deepEqual(
    provider.requests[0]?.tools?.map((tool) => tool.name),
    ['find', 'grep', 'tree'],
  );

  for (const request of provider.requests.slice(1)) {
    assert.equal(request.tools, undefined);
  }

  await rm(root, { recursive: true, force: true });
});

test('does not write PROMPT.md during prompt execution', async () => {
  const root = await workspace('prompt-no-write');

  await prompt(createPromptArtifact('Do not persist yet.'), {
    ...workflowOptions(
      createProvider(responsesWithoutQuestions()).provider,
      root,
    ),
  });

  await assert.rejects(access(path.join(root, 'PROMPT.md')));
  await rm(root, { recursive: true, force: true });
});

test('fails clearly when a pass does not return structured output', async () => {
  const root = await workspace('prompt-missing-structured-output');

  await assert.rejects(
    prompt(createPromptArtifact('Invalid JSON.'), {
      ...workflowOptions(createUnstructuredProvider('not json').provider, root),
    }),
    /Exploration pass did not return structured output/,
  );
  await rm(root, { recursive: true, force: true });
});

test('fails clearly when an open question contains unexpected fields', async () => {
  const root = await workspace('prompt-open-question-extra-field');
  const provider = createProvider([
    exploration(),
    requestUnderstanding(),
    intent(),
    {
      questions: [
        {
          question: 'Question?',
          recommendation: 'Recommendation.',
          options: [
            'Use the first solution.',
            'Use the second solution.',
            'Use the third solution.',
          ],
          owner: 'Unexpected owner field.',
        },
      ],
    },
  ]);

  await assert.rejects(
    prompt(createPromptArtifact('Invalid open question.'), {
      ...workflowOptions(provider.provider, root),
    }),
    /Fake provider response 3 failed schema validation/,
  );
  await rm(root, { recursive: true, force: true });
});

test('fails clearly when an open question has fewer than three options', async () => {
  const root = await workspace('prompt-open-question-few-options');
  const provider = createProvider([
    exploration(),
    requestUnderstanding(),
    intent(),
    {
      questions: [
        {
          question: 'Question?',
          recommendation: 'Use the first solution.',
          options: ['Use the first solution.', 'Use the second solution.'],
        },
      ],
    },
  ]);

  await assert.rejects(
    prompt(createPromptArtifact('Invalid open question options.'), {
      ...workflowOptions(provider.provider, root),
    }),
    /Fake provider response 3 failed schema validation/,
  );
  await rm(root, { recursive: true, force: true });
});

test('ignores repository markdown fixtures unless tools discover them', async () => {
  const root = await workspace('prompt-self-contained');
  await write(root, 'GROUNDING.md', 'GROUNDING_SENTINEL');
  await write(root, 'AGENTS.md', 'AGENTS_SENTINEL');
  await write(root, 'README.md', 'README_SENTINEL');
  await write(
    root,
    'docs/01-prompt/01-prompt-ingestion-grounding-architecture.md',
    'DOCS_SENTINEL',
  );
  const provider = createProvider(responsesWithoutQuestions());

  await prompt(createPromptArtifact('Use embedded pass instructions.'), {
    ...workflowOptions(provider.provider, root),
  });

  const text = messageText(provider);

  assert.equal(text.includes('GROUNDING_SENTINEL'), false);
  assert.equal(text.includes('AGENTS_SENTINEL'), false);
  assert.equal(text.includes('README_SENTINEL'), false);
  assert.equal(text.includes('DOCS_SENTINEL'), false);
  assert.equal(text.includes('GROUNDING.md'), false);
  assert.equal(text.includes('AGENTS.md'), false);
  assert.equal(text.includes('docs/01-prompt'), false);
  assert.equal(text.includes('Return strict JSON'), false);
  await rm(root, { recursive: true, force: true });
});

const workspace = async (name: string): Promise<string> =>
  mkdtemp(path.join(os.tmpdir(), `doric-${name}-`));

const write = async (
  root: string,
  file: string,
  content: string,
): Promise<void> => {
  const target = path.join(root, file);
  await mkdir(path.dirname(target), { recursive: true });
  await writeFile(target, content, 'utf8');
};
