import { createHash, randomBytes } from 'node:crypto';

import type { OAuthRandomSource } from '../types/oauth.js';

export const nodeRandomSource: OAuthRandomSource = {
  bytes(length: number): Uint8Array {
    return randomBytes(length);
  },
};

export const token = (random: OAuthRandomSource, length: number): string =>
  base64Url(random.bytes(length));

export const challenge = (verifier: string): string =>
  base64Url(createHash('sha256').update(verifier).digest());

export const base64Url = (value: Uint8Array): string =>
  Buffer.from(value).toString('base64url');
