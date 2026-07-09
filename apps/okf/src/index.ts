import assert from 'node:assert';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

import { Command } from 'commander';

import pino from 'pino';
import pretty from 'pino-pretty';

import { analyze } from './lib/agents/analyze/index.js';
import { classify } from './lib/agents/classify/index.js';
import { frontmatter } from './lib/agents/frontmatter/index.js';
import { indexing } from './lib/agents/indexing/index.js';
import { walk } from './lib/walk.js';

const logger = pino(pretty());

/** Runs the OKF CLI against the repository path passed through --path. */
async function main(): Promise<void> {
  const program = new Command()
    .name('okf')
    .description('Generate an Open Knowledge Format bundle for a repository.')
    .requiredOption('--path <repo-root>', 'repository root to analyze');

  program.parse();

  const options = program.opts<{
    readonly path: string;
  }>();

  logger.info(
    {
      path: options.path,
    },
    'okf.start',
  );

  const root = options.path;
  const output = path.join(options.path, '.doric', 'knowledge');

  // Ensure the root folder is created
  try {
    await fs.promises.access(output, fs.constants.F_OK);
    logger.info(
      {
        path: output,
      },
      'okf:output folder already exists',
    );
  } catch {
    await fs.promises.mkdir(output, { recursive: true });
    logger.info(
      {
        path: output,
      },
      'okf:output folder created',
    );
  }

  // Root .gitignore

  // We walk file by file
  await walk(
    root,
    {
      file: async (file, body) => {
        const markdown = await work(root, output, file, body);

        if (!markdown) return '';

        return markdown;
      },
      folder: async (folderRoot, files) => {
        const indexRoot = path.join(output, path.relative(root, folderRoot));
        const documents = (
          await Promise.all(
            files.map(async (file) => {
              const relative = relativeFile(root, file.path);
              const target = path.join(output, `${relative}.md`);
              const content =
                file.output.trim().length > 0
                  ? file.output
                  : await readFileIfExists(target);
              const body = content.trim();

              if (body.length === 0) {
                return;
              }

              return [
                `Source: ${toPosix(relative)}`,
                `Link: ${toPosix(path.relative(indexRoot, target))}`,
                '',
                body,
              ].join('\n');
            }),
          )
        ).filter((document): document is string => document !== undefined);

        if (documents.length === 0) {
          return;
        }

        const response = await indexing(folderRoot, documents);
        const markdown = response.text.trim();

        if (markdown.length === 0) {
          return;
        }

        const target = path.join(indexRoot, 'index.md');

        await fs.promises.mkdir(indexRoot, { recursive: true });
        await fs.promises.writeFile(target, `${markdown}\n`, 'utf-8');

        logger.info(
          {
            path: target,
          },
          'okf:index written',
        );
      },
    },
    {
      ignore: [
        // LLM instructions
        '.agents',

        // Config files
        'package-lock.json',

        // Forbidden EXT.
        '*.pdf',
        '*.exe',
        '*.msi',
      ],
    },
  );
}

const work = async (
  root: string,
  outputRoot: string,
  file: string,
  body: string,
) => {
  logger.info(
    {
      path: file,
    },
    'okf:working',
  );

  // Generate a hash for the file content
  const hash = crypto.createHash('sha256').update(body).digest('hex');
  const relative = relativeFile(root, file);
  const target = path.join(outputRoot, `${relative}.md`);

  if (await containsHash(target, hash)) {
    logger.info(
      {
        hash,
        path: file,
        target,
      },
      'okf:skipped',
    );

    return;
  }

  const classification = await classify(file, body);

  assert(
    classification.structured,
    `Classification did not return structured output for ${file}`,
  );

  const front = await frontmatter(file, body);

  assert(
    front.structured,
    `Front did not return structured output for ${file}`,
  );

  const analysis = await analyze(file, body, classification.structured);

  assert(
    analysis.structured,
    `Analysis did not return structured output for ${file}`,
  );

  const markdown = `---
type: ${yamlString(front.structured.type)}
title: ${yamlString(front.structured.title)}
description: ${yamlString(front.structured.description)}
resource: ${yamlString(`source:${toPosix(relative)}`)}
tags: [${front.structured.tags.map(yamlString).join(', ')}]
timestamp: ${yamlString(new Date().toISOString())}
hash: ${yamlString(hash)}
---

${analysis.structured.summary}
`;

  await fs.promises.mkdir(path.dirname(target), { recursive: true });
  await fs.promises.writeFile(target, markdown, 'utf-8');

  logger.info(
    {
      path: target,
      hash,
    },
    'okf:written',
  );

  return markdown;
};

const relativeFile = (root: string, file: string): string => {
  const relative = path.relative(root, file);

  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    throw new Error(
      `Cannot write output for file outside repository root: ${file}`,
    );
  }

  return relative;
};

const toPosix = (value: string): string => value.split(path.sep).join('/');

const yamlString = (value: string): string => JSON.stringify(value);

const readFileIfExists = async (file: string): Promise<string> => {
  try {
    return await fs.promises.readFile(file, 'utf-8');
  } catch (error) {
    if (isMissingFile(error)) {
      return '';
    }

    throw error;
  }
};

const containsHash = async (file: string, hash: string): Promise<boolean> => {
  try {
    return (await fs.promises.readFile(file, 'utf-8')).includes(hash);
  } catch (error) {
    if (isMissingFile(error)) {
      return false;
    }

    throw error;
  }
};

const isMissingFile = (error: unknown): error is NodeJS.ErrnoException =>
  error instanceof Error &&
  'code' in error &&
  (error as NodeJS.ErrnoException).code === 'ENOENT';

main()
  .then(() => {
    logger.info('Finished!');
  })
  .catch((error) => logger.error(error.message));
