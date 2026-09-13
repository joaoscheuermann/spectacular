#!/usr/bin/env node
import { writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

import { withSandbox } from './environment.mjs';
import { run } from './flow.mjs';
import { createRuntime } from './runtime.mjs';
import { prepareRun } from './stages/prepare.mjs';

const readOptions = () =>
  parseArgs({
    options: {
      request: { type: 'string' },
      workspace: { type: 'string' },
      skills: {
        type: 'string',
        default: fileURLToPath(
          new URL('../skill-retrieval-gating/cases/skills', import.meta.url),
        ),
      },
      model: { type: 'string', default: 'deepseek/deepseek-v4.1-flash' },
      'execution-model': {
        type: 'string',
        default: 'deepseek/deepseek-v4.1-flash',
      },
      'criteria-model': {
        type: 'string',
        default: 'deepseek/deepseek-v4.1-flash',
      },
      'judge-model': {
        type: 'string',
        default: 'deepseek/deepseek-v4.1-flash',
      },
      prepare: { type: 'boolean', default: false },
      help: { type: 'boolean', default: false },
    },
    strict: true,
    allowPositionals: false,
  }).values;

const help = [
  'Usage: npm run llm:mosaic-e2e -- [options]',
  'Without arguments, run the bundled CSV reconciliation task in a fresh workspace.',
  '--request FILE        Markdown request (provide --workspace with task inputs)',
  '--workspace DIR       Local input directory copied into the Docker sandbox',
  '--skills DIR          Flat Markdown skill catalog (default: retrieval lab catalog)',
  '--model ID            Planning and skill-selection model',
  '--execution-model ID  Node execution model (low reasoning effort)',
  '--criteria-model ID   Criteria model',
  '--judge-model ID      Completion judge model',
  '--prepare             Prepare and validate local inputs without provider calls',
].join('\n');

const configuration = (options) => ({
  planningModel: options.model,
  executionModel: options['execution-model'],
  criteriaModel: options['criteria-model'],
  judgeModel: options['judge-model'],
  effort: 'high',
  executionEffort: 'low',
  embeddingModel: 'voyageai/voyage-4-large',
  embeddingDimensions: 1024,
  rerankerModel: 'voyageai/rerank-2.5',
  retrievalK: 20,
  topK: 10,
  maxAttempts: 2,
  maxTurns: 20,
  sandbox: {
    image: 'node:22-bookworm',
    resources: { cpuCount: 1, memoryMiB: 512, diskMiB: 4096 },
  },
});

const saveJson = (path, value) =>
  writeFile(path, JSON.stringify(value, null, 2) + '\n');

const saveResult = async ({ output, manifest }, result, usage) => {
  await saveJson(join(output, 'results.json'), { ...result, usage });

  if (result.delivery !== null) {
    await writeFile(join(output, 'delivery.md'), result.delivery);
  }

  await saveJson(join(output, 'manifest.json'), {
    ...manifest,
    status: result.status,
    completedAt: new Date().toISOString(),
  });
};

const executeRun = async (prepared) => {
  const { output, manifest } = prepared;
  const { id, request, skills, workspace, config } = manifest;
  let runtime;

  try {
    const result = await withSandbox(
      { workspace, output, config },
      async (sandbox) => {
        runtime = createRuntime({
          directory: output,
          sandbox,
          config,
          core: prepared.core,
        });

        await runtime.record('sandbox', {
          id: sandbox.id,
          root: sandbox.root,
          image: config.sandbox.image,
        });

        return run(runtime, { request, skills });
      },
    );

    await saveResult(prepared, result, runtime.usage);

    runtime.logger.info(
      {
        runId: id,
        status: result.status,
        metrics: result.metrics,
        providerCalls: runtime.usage.length,
      },
      'Run completed',
    );

    if (result.status !== 'completed') {
      process.exitCode = 1;
    }
  } catch (error) {
    const code = error.data?.code ?? error.code ?? error.name;

    await saveJson(join(output, 'manifest.json'), {
      ...manifest,
      status: 'failed',
      code,
      completedAt: new Date().toISOString(),
      usage: runtime?.usage ?? [],
    });

    console.error(
      'Run failed (' + code + '); inspect ' + output + '/trace.jsonl',
    );

    process.exitCode = 1;
  }
};

const main = async () => {
  const options = readOptions();

  if (options.help) {
    console.log(help);

    return;
  }

  const prepared = await prepareRun({
    options,
    config: configuration(options),
  });

  if (options.prepare) {
    return;
  }

  await executeRun(prepared);
};

await main();
