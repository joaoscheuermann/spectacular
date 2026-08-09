import pino from 'pino';

import {
  EMBEDDING_MODEL,
  PRIMARY_MODEL,
  RERANKER_MODEL,
} from '../config/index.js';
import type { CliInvocation } from './args.js';
import { booleanFlag, rejectUnknownFlags, requiredFlag } from './args.js';
import { buildProductionIndex } from './index-lifecycle.js';
import { readJson } from './io.js';
import { createMeteringStore } from './metering-store.js';
import { paidCapabilityProbes, validateFreeCapabilities } from './preflight.js';
import { parsePrices, validatePriceCoverage } from './pricing.js';
import { createStudyProvider } from './provider-factory.js';

const requirements = [
  { model: PRIMARY_MODEL.model, kind: 'completion' },
  { model: EMBEDDING_MODEL.model, kind: 'embedding' },
  { model: RERANKER_MODEL, kind: 'rerank' },
] as const;

/** Builds the paid setup artifact only behind both explicit acknowledgements. */
export const buildIndex = async (
  invocation: CliInvocation,
): Promise<unknown> => {
  rejectUnknownFlags(invocation, [
    'artifacts',
    'candidate-model',
    'prices',
    'yes-paid-probes',
    'yes-paid-setup',
  ]);
  if (!booleanFlag(invocation, 'yes-paid-setup')) {
    throw new TypeError('index construction requires --yes-paid-setup');
  }
  if (!booleanFlag(invocation, 'yes-paid-probes')) {
    throw new TypeError('capability probes require --yes-paid-probes');
  }
  const prices = parsePrices(
    await readJson(requiredFlag(invocation, 'prices')),
  );
  const candidateModel = requiredFlag(invocation, 'candidate-model');
  validatePriceCoverage(prices, [
    ...requirements,
    { model: candidateModel, kind: 'completion' },
  ]);
  const apiKey = process.env['OPENROUTER_API_KEY'];
  if (apiKey === undefined || apiKey.trim().length === 0) {
    throw new Error('OPENROUTER_API_KEY is required');
  }
  const root = requiredFlag(invocation, 'artifacts');
  const logger = pino({ level: 'silent' });
  const meter = createStudyProvider(apiKey, prices, logger);
  const capability = await validateFreeCapabilities(
    meter.provider,
    candidateModel,
  );
  const result = await buildProductionIndex({
    root,
    provider: meter.provider,
    meter,
    metering: createMeteringStore(root),
    probes: paidCapabilityProbes(meter.provider),
  });
  return {
    indexHash: result.artifact.indexHash,
    path: result.path,
    reused: result.reused,
    attempt: result.attempt,
    setupUsage: result.usage,
    capability,
  };
};
