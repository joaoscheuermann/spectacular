import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rm,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';
import type { TestContext } from 'node:test';

import {
  SUMMARIZER_STYLE_RULES,
  createStaticSummarizer,
  generateKnowledgeBundle,
} from '../src/index.js';
import type { KnowledgeLogger, KnowledgeSummarizer } from '../src/index.js';

test('writes knowledge under .doric and ignores .agents .doric gitignore matches and binary files', async (t) => {
  const root = await tempRepo(t);
  const source = 'export const value = 1;\n';
  const ignored = 'ignore me\n';
  const doricCache = 'old knowledge output\n';
  const binary = Buffer.from([0, 1, 2, 3, 4, 5]);

  await writeText(root, '.gitignore', 'ignored.txt\nignored-dir/\n');
  await writeText(root, 'src/app.ts', source);
  await writeText(root, 'ignored.txt', ignored);
  await writeText(
    root,
    'ignored-dir/file.ts',
    'export const ignored = true;\n',
  );
  await writeText(root, '.agents/existing.md', 'existing knowledge\n');
  await writeText(root, '.doric/cache.md', doricCache);
  await writeBinary(root, 'asset.exe', binary);

  const result = await generateKnowledgeBundle({
    rootPath: root,
    summarizer: createStaticSummarizer(),
    clock: fixedClock('2026-07-05T00:00:00.000Z'),
  });

  assert.equal(
    await exists(knowledgePath(root, 'src', `${hash(source)}.md`)),
    true,
  );
  assert.equal(await exists(knowledgePath(root, `${hash(ignored)}.md`)), false);
  assert.equal(await exists(knowledgePath(root, 'ignored-dir')), false);
  assert.equal(await exists(knowledgePath(root, '.agents')), false);
  assert.equal(await exists(knowledgePath(root, '.doric')), false);
  assert.equal(
    await exists(knowledgePath(root, `${hash(doricCache)}.md`)),
    false,
  );
  assert.equal(await exists(knowledgePath(root, `${hash(binary)}.md`)), false);
  assert.equal(result.knowledgePath, path.join(root, '.doric', 'knowledge'));
  assert.deepEqual(result.skippedFiles, [
    { relativePath: 'asset.exe', reason: 'binary' },
  ]);
});

test('emits concise progress logs through the injected logger', async (t) => {
  const root = await tempRepo(t);
  const entries: {
    readonly details: Record<string, unknown>;
    readonly message: string;
  }[] = [];
  const logger: KnowledgeLogger = {
    info: (details, message) => {
      entries.push({ details: { ...details }, message });
    },
  };

  await writeText(root, 'src/app.ts', 'export const value = 1;\n');
  await generateKnowledgeBundle({
    rootPath: root,
    summarizer: createStaticSummarizer(),
    logger,
    clock: fixedClock('2026-07-05T00:15:00.000Z'),
  });

  assert.deepEqual(
    entries.map((entry) => entry.message),
    [
      'okf.start',
      'okf.collect',
      'okf.files',
      'okf.summarize.files',
      'okf.link.relationships',
      'okf.complete',
    ],
  );
  assert.equal(entries.at(-1)?.details['knowledgePath'], knowledgePath(root));
});

test('keeps summarizer prompts grounded and plain', () => {
  assert.match(SUMMARIZER_STYLE_RULES, /Do not use emoji/u);
  assert.match(SUMMARIZER_STYLE_RULES, /Do not hallucinate/u);
  assert.match(SUMMARIZER_STYLE_RULES, /Do not make assumptions/u);
  assert.match(SUMMARIZER_STYLE_RULES, /Use only information/u);
  assert.doesNotMatch(SUMMARIZER_STYLE_RULES, /[\u{1F300}-\u{1FAFF}]/u);
});

test('honors nested gitignore rules for descendant files', async (t) => {
  const root = await tempRepo(t);
  const kept = 'export const kept = true;\n';
  const ignored = 'export const ignored = true;\n';

  await writeText(root, '.gitignore', 'root-ignored.txt\n');
  await writeText(root, 'root-ignored.txt', 'ignored at root\n');
  await writeText(root, 'src/.gitignore', 'ignored.py\ncache/\n');
  await writeText(root, 'src/kept.py', kept);
  await writeText(root, 'src/ignored.py', ignored);
  await writeText(root, 'src/cache/generated.py', ignored);
  await run(root, '2026-07-05T00:30:00.000Z');

  assert.equal(
    await exists(knowledgePath(root, 'src', `${hash(kept)}.md`)),
    true,
  );
  assert.equal(
    await exists(knowledgePath(root, 'src', `${hash(ignored)}.md`)),
    false,
  );
  assert.equal(await exists(knowledgePath(root, 'src', 'cache')), false);
});

test('writes hashed file concepts and preserves log history across reruns', async (t) => {
  const root = await tempRepo(t);
  const first = 'export const version = 1;\n';
  const second = 'export const version = 2;\n';
  const firstHash = hash(first);
  const secondHash = hash(second);

  await writeText(root, 'src/version.ts', first);
  await run(root, '2026-07-05T01:00:00.000Z');
  await run(root, '2026-07-05T02:00:00.000Z');

  const unchangedLog = await readKnowledge(root, 'src', 'log.md');
  assert.equal(
    await exists(knowledgePath(root, 'src', `${firstHash}.md`)),
    true,
  );
  assert.equal(occurrences(unchangedLog, firstHash), 1);
  assert.match(unchangedLog, /2026-07-05T01:00:00.000Z/u);
  assert.doesNotMatch(unchangedLog, /2026-07-05T02:00:00.000Z/u);

  await writeText(root, 'src/version.ts', second);
  await run(root, '2026-07-05T03:00:00.000Z');

  const changedLog = await readKnowledge(root, 'src', 'log.md');
  assert.equal(
    await exists(knowledgePath(root, 'src', `${secondHash}.md`)),
    true,
  );
  assert.match(changedLog, new RegExp(firstHash, 'u'));
  assert.match(changedLog, new RegExp(secondHash, 'u'));
  assert.match(changedLog, /2026-07-05T03:00:00.000Z/u);
});

test('extracts top-level keys from JSONC YAML and TOML config files', async (t) => {
  const root = await tempRepo(t);
  const jsonc = [
    '{',
    '  // compiler settings',
    '  "compilerOptions": { "strict": true, },',
    '  "include": ["src"],',
    '}',
    '',
  ].join('\n');
  const yaml = [
    'extends: ./base.yaml',
    'rules:',
    '  semi: error',
    'plugins:',
    '  - local',
    '',
  ].join('\n');
  const toml = [
    'title = "Example"',
    '[package]',
    'name = "demo"',
    '[dependencies]',
    'serde = "1"',
    '',
  ].join('\n');

  await writeText(root, 'tsconfig.jsonc', jsonc);
  await writeText(root, '.github/workflows/ci.yaml', yaml);
  await writeText(root, 'pyproject.toml', toml);
  await run(root, '2026-07-05T03:30:00.000Z');

  const jsoncDoc = await readKnowledge(root, `${hash(jsonc)}.md`);
  const yamlDoc = await readKnowledge(
    root,
    '.github',
    'workflows',
    `${hash(yaml)}.md`,
  );
  const tomlDoc = await readKnowledge(root, `${hash(toml)}.md`);

  assert.match(jsoncDoc, /- `compilerOptions`/u);
  assert.match(jsoncDoc, /- `include`/u);
  assert.match(yamlDoc, /- `extends`/u);
  assert.match(yamlDoc, /- `rules`/u);
  assert.match(yamlDoc, /- `plugins`/u);
  assert.match(tomlDoc, /- `title`/u);
  assert.match(tomlDoc, /- `package`/u);
  assert.match(tomlDoc, /- `dependencies`/u);
});

test('extracts package configuration and code declarations', async (t) => {
  const root = await tempRepo(t);
  const manifest = `${JSON.stringify(
    {
      scripts: { build: 'tsc -p tsconfig.json', test: 'node --test' },
      dependencies: { react: '^19.0.0' },
      devDependencies: { vitest: '^3.0.0' },
    },
    null,
    2,
  )}\n`;
  const code = [
    "import { helper } from './helper.js';",
    'export interface Options { readonly name: string }',
    'export function run(options: Options) { return helper(options.name); }',
    'export class Runner {',
    '  execute() { return run({ name: "demo" }); }',
    '}',
    '',
  ].join('\n');

  await writeText(root, 'package.json', manifest);
  await writeText(root, 'src/main.ts', code);
  await writeText(
    root,
    'src/helper.ts',
    'export const helper = (name: string) => name;\n',
  );
  await run(root, '2026-07-05T04:00:00.000Z');

  const packageDoc = await readKnowledge(root, `${hash(manifest)}.md`);
  const codeDoc = await readKnowledge(root, 'src', `${hash(code)}.md`);

  assert.match(packageDoc, /# Package Scripts/u);
  assert.match(packageDoc, /`build`/u);
  assert.match(packageDoc, /# Dependencies/u);
  assert.match(packageDoc, /`react`/u);
  assert.match(packageDoc, /# Dev Dependencies/u);
  assert.match(packageDoc, /`vitest`/u);
  assert.match(codeDoc, /# Exports/u);
  assert.match(codeDoc, /`run` \(function\)/u);
  assert.match(codeDoc, /`Options` \(interface\)/u);
  assert.match(codeDoc, /# Imports/u);
  assert.match(codeDoc, /`\.\/helper\.js`/u);
});

test('uses generated summary text for file frontmatter descriptions', async (t) => {
  const root = await tempRepo(t);
  const source = 'export function run() { return 1; }\n';
  const summarizer: KnowledgeSummarizer = {
    summarizeFile: async () => ({
      body: [
        '# Summary',
        '',
        'Exports the run function used by callers. It returns a numeric value.',
      ].join('\n'),
    }),
    summarizeFolder: async () => ({
      body: '# Folder Summary\n\nUses direct file summaries.',
    }),
  };

  await writeText(root, 'src/run.ts', source);
  await generateKnowledgeBundle({
    rootPath: root,
    summarizer,
    clock: fixedClock('2026-07-05T04:15:00.000Z'),
  });

  const doc = await readKnowledge(root, 'src', `${hash(source)}.md`);

  assert.match(
    doc,
    /description: "Exports the run function used by callers."/u,
  );
});

test('describes common package dependency families deterministically', async (t) => {
  const root = await tempRepo(t);
  const manifest = `${JSON.stringify(
    {
      dependencies: {
        commander: '^15.0.0',
        glob: '^13.0.6',
        ignore: '^5.3.2',
        tslib: '^2.8.0',
      },
      devDependencies: {
        '@nx/js': '^22.0.0',
        '@swc/core': '^1.15.0',
        '@types/node': '^24.0.0',
        prettier: '^3.0.0',
        typescript: '^5.9.0',
      },
    },
    null,
    2,
  )}\n`;

  await writeText(root, 'package.json', manifest);
  await run(root, '2026-07-05T04:30:00.000Z');

  const packageDoc = await readKnowledge(root, `${hash(manifest)}.md`);

  assert.match(packageDoc, /CLI command parser/u);
  assert.match(packageDoc, /File globbing/u);
  assert.match(packageDoc, /Gitignore-compatible/u);
  assert.match(packageDoc, /TypeScript runtime helper/u);
  assert.match(packageDoc, /Nx plugin/u);
  assert.match(packageDoc, /SWC compiler/u);
  assert.match(packageDoc, /TypeScript declarations/u);
  assert.match(packageDoc, /code formatter/u);
  assert.match(packageDoc, /TypeScript compiler/u);
});

test('generates folder indexes from all direct file summaries', async (t) => {
  const root = await tempRepo(t);
  const first = 'export const a = 1;\n';
  const second = '# Notes\n';

  await writeText(root, 'src/a.ts', first);
  await writeText(root, 'src/b.md', second);
  await run(root, '2026-07-05T05:00:00.000Z');

  const index = await readKnowledge(root, 'src', 'index.md');

  assert.match(index, /Direct summaries: src\/a\.ts, src\/b\.md\./u);
  assert.match(
    index,
    new RegExp(`\\[src/a\\.ts\\]\\(${hash(first)}\\.md\\)`, 'u'),
  );
  assert.match(
    index,
    new RegExp(`\\[src/b\\.md\\]\\(${hash(second)}\\.md\\)`, 'u'),
  );
});

test('passes generated file summary bodies into folder summary generation', async (t) => {
  const root = await tempRepo(t);
  const first = 'export const a = 1;\n';
  const second = '# Notes\n';
  let srcFolderFiles:
    | readonly {
        readonly sourceRelativePath: string;
        readonly summaryBody: string;
      }[]
    | undefined;
  const summarizer: KnowledgeSummarizer = {
    summarizeFile: async (input) => ({
      description: `Description for ${input.file.relativePath}`,
      body: `Generated body for ${input.file.relativePath}`,
    }),
    summarizeFolder: async (input) => {
      if (input.relativePath === 'src') {
        srcFolderFiles = input.directFiles.map((file) => ({
          sourceRelativePath: file.sourceRelativePath,
          summaryBody: file.summaryBody,
        }));
      }

      return {
        description: `Folder ${input.relativePath || '.'}`,
        body: input.directFiles.map((file) => file.summaryBody).join('\n'),
      };
    },
  };

  await writeText(root, 'src/a.ts', first);
  await writeText(root, 'src/b.md', second);
  await generateKnowledgeBundle({
    rootPath: root,
    summarizer,
    clock: fixedClock('2026-07-05T05:30:00.000Z'),
  });

  const index = await readKnowledge(root, 'src', 'index.md');

  assert.deepEqual(srcFolderFiles, [
    {
      sourceRelativePath: 'src/a.ts',
      summaryBody: 'Generated body for src/a.ts',
    },
    {
      sourceRelativePath: 'src/b.md',
      summaryBody: 'Generated body for src/b.md',
    },
  ]);
  assert.match(index, /Generated body for src\/a\.ts/u);
  assert.match(index, /Generated body for src\/b\.md/u);
});

test('adds second-pass OKF links for related files after hashes are known', async (t) => {
  const root = await tempRepo(t);
  const importer =
    "import { value } from './target.js';\nexport const result = value;\n";
  const target = 'export const value = 42;\n';

  await writeText(root, 'src/importer.ts', importer);
  await writeText(root, 'src/target.ts', target);
  await run(root, '2026-07-05T06:00:00.000Z');

  const importerDoc = await readKnowledge(root, 'src', `${hash(importer)}.md`);

  assert.match(importerDoc, /# Relationships/u);
  assert.match(importerDoc, new RegExp(`\\(/src/${hash(target)}\\.md\\)`, 'u'));
  assert.match(importerDoc, /via `\.\/target\.js`/u);
});

test('adds second-pass OKF links for Python and Rust local relationships', async (t) => {
  const root = await tempRepo(t);
  const pythonImporter = [
    'from .helper import value',
    'import sibling',
    'result = value + sibling.extra',
    '',
  ].join('\n');
  const pythonHelper = 'value = 1\n';
  const pythonSibling = 'extra = 2\n';
  const rustImporter = [
    'mod local;',
    'use crate::shared::Thing;',
    'pub fn run() -> Thing { Thing }',
    '',
  ].join('\n');
  const rustLocal = 'pub fn local() {}\n';
  const rustShared = 'pub struct Thing;\n';

  await writeText(root, 'py/app.py', pythonImporter);
  await writeText(root, 'py/helper.py', pythonHelper);
  await writeText(root, 'py/sibling.py', pythonSibling);
  await writeText(root, 'src/lib.rs', rustImporter);
  await writeText(root, 'src/local.rs', rustLocal);
  await writeText(root, 'src/shared.rs', rustShared);
  await run(root, '2026-07-05T06:30:00.000Z');

  const pythonDoc = await readKnowledge(
    root,
    'py',
    `${hash(pythonImporter)}.md`,
  );
  const rustDoc = await readKnowledge(root, 'src', `${hash(rustImporter)}.md`);

  assert.match(
    pythonDoc,
    new RegExp(`\\(/py/${hash(pythonHelper)}\\.md\\)`, 'u'),
  );
  assert.match(
    pythonDoc,
    new RegExp(`\\(/py/${hash(pythonSibling)}\\.md\\)`, 'u'),
  );
  assert.match(rustDoc, new RegExp(`\\(/src/${hash(rustLocal)}\\.md\\)`, 'u'));
  assert.match(rustDoc, new RegExp(`\\(/src/${hash(rustShared)}\\.md\\)`, 'u'));
});

const run = async (root: string, timestamp: string): Promise<void> => {
  await generateKnowledgeBundle({
    rootPath: root,
    summarizer: createStaticSummarizer(),
    clock: fixedClock(timestamp),
  });
};

const tempRepo = async (t: TestContext): Promise<string> => {
  const root = path.join(tmpdir(), 'okf-');
  await mkdir(root, { recursive: true });
  const repo = await mkdtemp(path.join(root, 'repo-'));

  t.after(async () => {
    await rm(repo, { recursive: true, force: true });
  });

  return repo;
};

const writeText = async (
  root: string,
  relativePath: string,
  content: string,
): Promise<void> => {
  const absolutePath = path.join(root, ...relativePath.split('/'));
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, content, 'utf8');
};

const writeBinary = async (
  root: string,
  relativePath: string,
  content: Buffer,
): Promise<void> => {
  const absolutePath = path.join(root, ...relativePath.split('/'));
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, content);
};

const readKnowledge = async (
  root: string,
  ...parts: readonly string[]
): Promise<string> => readFile(knowledgePath(root, ...parts), 'utf8');

const knowledgePath = (root: string, ...parts: readonly string[]): string =>
  path.join(root, '.doric', 'knowledge', ...parts);

const exists = async (absolutePath: string): Promise<boolean> => {
  try {
    await readdir(absolutePath);
    return true;
  } catch {
    try {
      await readFile(absolutePath);
      return true;
    } catch {
      return false;
    }
  }
};

const hash = (content: string | Buffer): string =>
  createHash('sha256').update(content).digest('hex');

const occurrences = (value: string, pattern: string): number =>
  value.split(pattern).length - 1;

const fixedClock =
  (timestamp: string): (() => Date) =>
  () =>
    new Date(timestamp);
