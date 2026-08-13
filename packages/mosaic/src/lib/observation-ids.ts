import { randomUUID } from 'node:crypto';

const attempts = 32;
const format = /^[0-9a-f]{6}$/u;

export interface ObservationIdAllocator {
  next(): string;
  reserve(ids: Iterable<string>): void;
  claim(ids: Iterable<string>): void;
}

/** Allocates one run-local six-hex handle while rejecting bounded collisions. */
export const createObservationIdAllocator = (
  createUuid: () => string = randomUUID,
): ObservationIdAllocator => {
  const reserved = new Set<string>();

  return {
    next: () => {
      for (let attempt = 0; attempt < attempts; attempt += 1) {
        const id = createUuid().slice(0, 6).toLowerCase();
        if (!format.test(id)) {
          throw new Error(
            'Mosaic observation ID allocation returned an invalid UUID.',
          );
        }
        if (reserved.has(id)) continue;

        reserved.add(id);
        return id;
      }

      throw new Error(
        'Mosaic observation ID allocation exhausted after 32 collisions.',
      );
    },
    reserve: (ids) => {
      for (const id of ids) reserved.add(id);
    },
    claim: (ids) => {
      const claimed = [...ids];
      const seen = new Set<string>();
      for (const id of claimed) {
        if (!format.test(id)) {
          throw new Error(
            'Mosaic observation ID must use six lowercase hexadecimal characters.',
          );
        }
        if (reserved.has(id) || seen.has(id)) {
          throw new Error('Mosaic observation ID is already reserved.');
        }
        seen.add(id);
      }
      for (const id of claimed) reserved.add(id);
    },
  };
};
