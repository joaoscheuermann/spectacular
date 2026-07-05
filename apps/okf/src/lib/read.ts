import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';

export type TextReadResult =
  | {
      readonly type: 'text';
      readonly buffer: Buffer;
      readonly content: string;
    }
  | {
      readonly type: 'skipped';
      readonly reason: 'binary' | 'unreadable';
    };

export const readTextFile = async (
  absolutePath: string,
): Promise<TextReadResult> => {
  try {
    const buffer = await readFile(absolutePath);

    if (isProbablyBinary(buffer)) {
      return { type: 'skipped', reason: 'binary' };
    }

    return {
      type: 'text',
      buffer,
      content: buffer.toString('utf8'),
    };
  } catch {
    return { type: 'skipped', reason: 'unreadable' };
  }
};

export const hashContent = (buffer: Buffer): string =>
  createHash('sha256').update(buffer).digest('hex');

const isProbablyBinary = (buffer: Buffer): boolean => {
  if (buffer.length === 0) {
    return false;
  }

  const sample = buffer.subarray(0, Math.min(buffer.length, 8192));

  if (sample.includes(0)) {
    return true;
  }

  const controlBytes = [...sample].filter(isSuspiciousControlByte).length;
  const decoded = sample.toString('utf8');
  const replacementCharacters = [...decoded].filter(
    (char) => char === '\uFFFD',
  ).length;

  return controlBytes / sample.length > 0.3 || replacementCharacters > 2;
};

const isSuspiciousControlByte = (byte: number): boolean =>
  byte < 32 && byte !== 9 && byte !== 10 && byte !== 12 && byte !== 13;
