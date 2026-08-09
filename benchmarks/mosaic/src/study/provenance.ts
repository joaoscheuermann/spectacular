import { readFile } from 'node:fs/promises';

import { z } from 'zod';

import { SKILLS } from '../catalog/index.js';
import { CONDITIONS, loadConditionPrompts } from '../conditions/index.js';
import { TOOL_NAMES } from '../config/index.js';
import { artifactHash } from '../core/hash.js';
import { TOOL_CONTRACTS } from '../runtime/index.js';
import { schemasV1, type Case, type RunSpec } from '../schemas/index.js';
import { PILOT_CASES } from './cases.js';
import { SCHEDULE_SEED_SCOPE } from './scheduler.js';

const readBenchmarkFile = async (relativePath: string): Promise<string> => {
  const candidates = [
    new URL(`../../${relativePath}`, import.meta.url),
    new URL(`../../../${relativePath}`, import.meta.url),
  ];
  for (const candidate of candidates) {
    try {
      return await readFile(candidate, 'utf8');
    } catch (error) {
      if (
        typeof error !== 'object' ||
        error === null ||
        !('code' in error) ||
        error.code !== 'ENOENT'
      ) {
        throw error;
      }
    }
  }
  throw new Error(`benchmark provenance file is unavailable: ${relativePath}`);
};

const hashFiles = async (relativePaths: readonly string[]): Promise<string> =>
  artifactHash(
    Object.fromEntries(
      await Promise.all(
        relativePaths.map(async (path) => [
          path,
          await readBenchmarkFile(path),
        ]),
      ),
    ),
  );

const toolContracts = (): unknown =>
  TOOL_NAMES.map((name) => {
    const contract = TOOL_CONTRACTS[name];
    return {
      name,
      description: contract.description,
      inputSchema: z.toJSONSchema(contract.input, { io: 'output' }),
      outputSchema: z.toJSONSchema(contract.output, {
        io: 'output',
        unrepresentable: 'any',
      }),
    };
  });

const jsonSchemas = (): unknown =>
  Object.fromEntries(
    Object.entries(schemasV1).map(([name, schema]) => [
      name,
      z.toJSONSchema(schema, { io: 'input' }),
    ]),
  );

export interface LocalInstrumentHashes {
  readonly protocol: string;
  readonly schemas: string;
  readonly catalog: string;
  readonly tools: string;
  readonly pilotCases: string;
  readonly conditions: string;
  readonly prompts: string;
  readonly analysis: string;
  readonly renvLock: string;
}

/** Hashes every local implementation artifact represented in a freeze. */
export const localInstrumentHashes =
  async (): Promise<LocalInstrumentHashes> => {
    const prompts = await loadConditionPrompts();
    const [protocol, analysis, renvLock] = await Promise.all([
      hashFiles([
        'protocol/D01-D25.md',
        'analysis/PROTOCOL.md',
        'analysis/REVIEW.md',
      ]),
      hashFiles([
        'analysis/analyze.R',
        'analysis/fit.R',
        'analysis/power.R',
        'analysis/report.R',
        'analysis/utils.R',
        'analysis/validate.R',
      ]),
      readBenchmarkFile('analysis/renv.lock').then(artifactHash),
    ]);
    return {
      protocol,
      schemas: artifactHash(jsonSchemas()),
      catalog: artifactHash(SKILLS),
      tools: artifactHash(toolContracts()),
      pilotCases: artifactHash(PILOT_CASES),
      conditions: artifactHash(CONDITIONS),
      prompts: prompts.hash,
      analysis,
      renvLock,
    };
  };

export const confirmatoryCasesHash = (cases: readonly Case[]): string =>
  artifactHash(cases);

/** Hashes condition order and hook seeds; these never represent provider sampling. */
export const scheduleAndHookSeedLedgerHash = (
  schedule: readonly RunSpec[],
): string =>
  artifactHash({
    scope: SCHEDULE_SEED_SCOPE,
    runs: [...schedule]
      .sort((left, right) => left.order - right.order)
      .map((run) => ({
        studyId: run.studyId,
        caseId: run.caseId,
        conditionId: run.conditionId,
        repetition: run.repetition,
        hookSeed: run.seed,
        pairedBlock: run.pairedBlock,
        order: run.order,
      })),
  });

/** Version-one compatibility alias for the frozen artifact field `seeds`. */
export const seedLedgerHash = scheduleAndHookSeedLedgerHash;
