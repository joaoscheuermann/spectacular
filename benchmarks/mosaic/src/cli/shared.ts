import { readFile } from 'node:fs/promises';

import { sha256 } from '../core/index.js';
import type { CliInvocation } from './args.js';
import { optionalFlag, requiredFlag } from './args.js';
import { readJson, writeJsonExclusive } from './io.js';

export const input = async (invocation: CliInvocation): Promise<unknown> =>
  readJson(requiredFlag(invocation, 'input'));

export const outputOrValue = async (
  invocation: CliInvocation,
  value: unknown,
): Promise<unknown> => {
  const path = optionalFlag(invocation, 'output');
  return path === undefined ? value : writeJsonExclusive(path, value);
};

export const fileHash = async (path: string): Promise<string> =>
  `sha256:${sha256(await readFile(path))}`;

export const imageDigest = (reference: string): string =>
  reference.includes('@') ? (reference.split('@').at(-1) ?? '') : reference;
