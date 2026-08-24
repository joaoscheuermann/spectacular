import { createHash } from 'node:crypto';
import { mkdir, readFile, stat, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';

import { z } from 'zod';

import {
  parseSraCorpusJson,
  parseSraInstancesJson,
  type SraInstance,
} from './sra-fixtures.js';
import {
  aggregateSraRetrievalMetrics,
  type SraAggregateRetrievalMetrics,
  type SraRetrievalRecord,
} from './sra-metrics.js';
import {
  buildSraSkillGolds,
  selectSraPilot,
  SRA_BENCH_MANIFEST,
} from './sra-pilot.js';

const RankedSchema = z
  .object({
    skill_id: z.string().trim().min(1),
    score: z.number().finite().optional(),
  })
  .passthrough();
const RecordSchema = z
  .object({
    instance_id: z.string().trim().min(1),
    gold_skill_ids: z.array(z.string().trim().min(1)).min(1),
    retrieved: z.array(RankedSchema),
  })
  .passthrough();
const RetrievalSchema = z
  .object({ results: z.array(RecordSchema).min(1) })
  .passthrough();

export interface PrepareSraPilotOptions {
  readonly sourceRoot: string;
  readonly outputDir: string;
}

export interface SraPilotArtifactManifest {
  readonly schemaVersion: 1;
  readonly benchmark: 'SRA-Bench';
  readonly contract: typeof SRA_BENCH_MANIFEST;
  readonly selected: Readonly<Record<string, readonly string[]>>;
  readonly files: Readonly<
    Record<string, { readonly bytes: number; readonly sha256: string }>
  >;
}

/** Verifies the pinned public corpus and materializes the frozen 100-case pilot. */
export const prepareSraPilot = async (
  options: PrepareSraPilotOptions,
): Promise<SraPilotArtifactManifest> => {
  const sourceRoot = resolve(options.sourceRoot);
  const outputDir = resolve(options.outputDir);
  await requireAbsent(outputDir);
  const sources = SRA_BENCH_MANIFEST.dataset.artifacts;
  const corpusPath = join(sourceRoot, ...sources.corpus.path.split('/'));
  const instanceSources = await Promise.all(
    sources.instances.map(async (artifact) => {
      const path = join(sourceRoot, ...artifact.path.split('/'));
      const source = await verifiedSource(
        path,
        artifact.bytes,
        artifact.sha256,
      );
      return parseSraInstancesJson(source);
    }),
  );
  const corpusSource = await verifiedSource(
    corpusPath,
    sources.corpus.bytes,
    sources.corpus.sha256,
  );
  const corpus = parseSraCorpusJson(corpusSource);
  const pilot = selectSraPilot(instanceSources.flat());
  const gold = buildSraSkillGolds(pilot, corpus);
  const grouped = Object.fromEntries(
    SRA_BENCH_MANIFEST.pilot.datasets.map((dataset) => [
      dataset,
      pilot.filter((instance) => instance.dataset === dataset),
    ]),
  ) as Readonly<Record<string, readonly SraInstance[]>>;
  const outputs = {
    'instances/champ.json': `${JSON.stringify(grouped.champ, null, 2)}\n`,
    'instances/bigcodebench.json': `${JSON.stringify(grouped.bigcodebench, null, 2)}\n`,
    'gold.json': `${JSON.stringify(gold, null, 2)}\n`,
  };
  const files = Object.fromEntries(
    Object.entries(outputs).map(([path, source]) => [
      path,
      { bytes: Buffer.byteLength(source), sha256: sha256(source) },
    ]),
  );
  const manifest: SraPilotArtifactManifest = {
    schemaVersion: 1,
    benchmark: 'SRA-Bench',
    contract: SRA_BENCH_MANIFEST,
    selected: Object.fromEntries(
      Object.entries(grouped).map(([dataset, instances]) => [
        dataset,
        instances.map(({ instance_id: id }) => id),
      ]),
    ),
    files,
  };
  const persisted = {
    ...outputs,
    'manifest.json': `${JSON.stringify(manifest, null, 2)}\n`,
  };
  await Promise.all(
    Object.entries(persisted).map(async ([path, source]) => {
      const target = join(outputDir, ...path.split('/'));
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, source, 'utf8');
    }),
  );
  return manifest;
};

/** Parses the interoperable SR-Agents retrieval JSON contract. */
export const parseSraRetrievalResults = (
  source: string,
): readonly SraRetrievalRecord[] => {
  let value: unknown;
  try {
    value = JSON.parse(source) as unknown;
  } catch {
    throw new Error('SRA retrieval result is not valid JSON.');
  }
  const records = RetrievalSchema.parse(value).results;
  const ids = records.map(({ instance_id: id }) => id);
  if (new Set(ids).size !== ids.length)
    throw new Error('SRA retrieval result contains duplicate instances.');
  return records;
};

/** Scores one official retrieval artifact and optionally persists the report. */
export const scoreSraRetrievalFile = async (
  inputPath: string,
  k: number,
  outputPath?: string,
): Promise<SraAggregateRetrievalMetrics> => {
  const records = parseSraRetrievalResults(await readFile(inputPath, 'utf8'));
  const result = aggregateSraRetrievalMetrics(records, k);
  if (outputPath !== undefined) {
    await mkdir(dirname(resolve(outputPath)), { recursive: true });
    await writeFile(outputPath, `${JSON.stringify(result, null, 2)}\n`, 'utf8');
  }
  return result;
};

const verifiedSource = async (
  path: string,
  expectedBytes: number,
  expectedSha256: string,
): Promise<string> => {
  const data = await readFile(path);
  if (data.byteLength !== expectedBytes || sha256(data) !== expectedSha256) {
    throw new Error(`SRA-Bench artifact does not match its pin: ${path}`);
  }
  return data.toString('utf8');
};

const requireAbsent = async (path: string): Promise<void> => {
  try {
    await stat(path);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return;
    throw error;
  }
  throw new Error(`Output directory already exists: ${path}`);
};

const sha256 = (value: string | Buffer): string =>
  createHash('sha256').update(value).digest('hex');
