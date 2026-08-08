import { createHash } from 'node:crypto';

import { canonicalJson } from './json.js';

/** Hashes bytes as a lowercase SHA-256 digest. */
export const sha256 = (value: string | Uint8Array): string =>
  createHash('sha256').update(value).digest('hex');

/** Hashes a value after recursive object-key canonicalization. */
export const contentHash = (value: unknown): string =>
  sha256(canonicalJson(value));

/** Hashes a canonical value using the manifest-compatible URI form. */
export const artifactHash = (value: unknown): string =>
  `sha256:${contentHash(value)}`;

/** Creates a stable run identity from its complete public specification. */
export const runId = (spec: unknown): string => `run-${contentHash(spec)}`;
