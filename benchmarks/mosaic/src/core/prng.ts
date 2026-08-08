import { sha256 } from './hash.js';

export interface Prng {
  readonly next: () => number;
  readonly integer: (maxExclusive: number) => number;
  readonly shuffle: <T>(values: readonly T[]) => readonly T[];
  readonly fork: (label: string) => Prng;
}

const seedWords = (seed: string): readonly number[] => {
  const digest = sha256(seed);
  return [0, 8, 16, 24].map((offset) =>
    Number.parseInt(digest.slice(offset, offset + 8), 16),
  );
};

const rotateLeft = (value: number, bits: number): number =>
  ((value << bits) | (value >>> (32 - bits))) >>> 0;

const xoshiro = (initial: readonly number[]): (() => number) => {
  const state = [...initial] as [number, number, number, number];
  return () => {
    const result = Math.imul(rotateLeft(Math.imul(state[1], 5), 7), 9) >>> 0;
    const temporary = (state[1] << 9) >>> 0;
    state[2] ^= state[0];
    state[3] ^= state[1];
    state[1] ^= state[2];
    state[0] ^= state[3];
    state[2] ^= temporary;
    state[3] = rotateLeft(state[3], 11);
    return result / 0x1_0000_0000;
  };
};

/** Creates a deterministic, platform-independent seeded random source. */
export const createPrng = (seed: string): Prng => {
  const next = xoshiro(seedWords(seed));
  const integer = (maxExclusive: number): number => {
    if (!Number.isSafeInteger(maxExclusive) || maxExclusive <= 0) {
      throw new TypeError('maxExclusive must be a positive safe integer');
    }
    return Math.floor(next() * maxExclusive);
  };
  const shuffle = <T>(values: readonly T[]): readonly T[] => {
    const result = [...values];
    for (let index = result.length - 1; index > 0; index -= 1) {
      const selected = integer(index + 1);
      [result[index], result[selected]] = [
        result[selected] as T,
        result[index] as T,
      ];
    }
    return result;
  };
  return {
    next,
    integer,
    shuffle,
    fork: (label) => createPrng(`${seed}\u0000${label}`),
  };
};
