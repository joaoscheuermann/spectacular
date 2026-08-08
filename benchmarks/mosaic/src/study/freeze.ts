import { mkdir, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';

import { FreezeManifestV1, type FreezeManifest } from '../schemas/index.js';
import { artifactHash } from '../core/hash.js';
import { canonicalJson } from '../core/json.js';

export type FreezeInput = Omit<FreezeManifest, 'manifestHash'>;

/** Validates and writes a freeze exactly once; an existing path is never replaced. */
export const writeFreeze = async (
  path: string,
  input: FreezeInput,
): Promise<FreezeManifest> => {
  if (!input.replication.approved) {
    throw new TypeError(
      'freeze requires an approved non-OpenAI replication candidate',
    );
  }
  const manifest = FreezeManifestV1.parse({
    ...input,
    manifestHash: artifactHash(input),
  });
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${canonicalJson(manifest)}\n`, {
    encoding: 'utf8',
    flag: 'wx',
    mode: 0o600,
  });
  return manifest;
};
