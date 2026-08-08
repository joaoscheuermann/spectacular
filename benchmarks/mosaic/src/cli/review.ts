import { z } from 'zod';

import { ReviewV1 } from '../schemas/index.js';
import {
  ingestReview,
  prepareReview,
  reviewStatus,
  type PreparedReview,
  type ReviewPrepareInput,
} from '../scoring/index.js';
import type { CliInvocation } from './args.js';
import { rejectUnknownFlags, requiredFlag } from './args.js';
import { objectValue, writeJsonExclusive } from './io.js';
import { input, outputOrValue } from './shared.js';

const prepare = async (invocation: CliInvocation): Promise<unknown> => {
  rejectUnknownFlags(invocation, ['input', 'assignments', 'key']);
  const prepared = prepareReview(
    (await input(invocation)) as ReviewPrepareInput,
  );
  const assignments = await writeJsonExclusive(
    requiredFlag(invocation, 'assignments'),
    { schemaVersion: 1, assignments: prepared.assignments },
  );
  const key = await writeJsonExclusive(requiredFlag(invocation, 'key'), {
    schemaVersion: 1,
    key: prepared.key,
  });
  return { assignments, key, count: prepared.assignments.length };
};

const prepared = (value: unknown): PreparedReview => {
  const body = objectValue(value, 'prepared review');
  if (
    body['schemaVersion'] !== 1 ||
    !Array.isArray(body['assignments']) ||
    !Array.isArray(body['key'])
  ) {
    throw new TypeError('prepared review is invalid');
  }
  return body as unknown as PreparedReview;
};

const ingest = async (invocation: CliInvocation): Promise<unknown> => {
  rejectUnknownFlags(invocation, ['input', 'output']);
  const body = objectValue(await input(invocation));
  const reviews = ingestReview(
    prepared(body['prepared']),
    z.array(ReviewV1).parse(body['existing']),
    ReviewV1.parse(body['incoming']),
  );
  return outputOrValue(invocation, reviews);
};

const status = async (invocation: CliInvocation): Promise<unknown> => {
  rejectUnknownFlags(invocation, ['input']);
  const body = objectValue(await input(invocation));
  return reviewStatus(
    prepared(body['prepared']),
    z.array(ReviewV1).parse(body['reviews']),
  );
};

export const review = async (invocation: CliInvocation): Promise<unknown> => {
  if (invocation.subcommand === 'prepare') return prepare(invocation);
  if (invocation.subcommand === 'ingest') return ingest(invocation);
  if (invocation.subcommand === 'status') return status(invocation);
  throw new TypeError('review requires prepare, ingest, or status');
};
