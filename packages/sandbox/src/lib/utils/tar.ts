import { Buffer } from 'node:buffer';

export type TarFile = {
  readonly name: string;
  readonly data: Uint8Array;
};

const blockSize = 512;

/** Packs a single regular file into a minimal ustar archive. */
export const packFile = (name: string, data: Uint8Array): Uint8Array => {
  const header = new Uint8Array(blockSize);
  const bodyPadding = padding(data.byteLength);
  const output = new Uint8Array(
    blockSize + data.byteLength + bodyPadding + blockSize * 2,
  );

  writeText(header, 0, 100, name);
  writeOctal(header, 100, 8, 0o644);
  writeOctal(header, 108, 8, 0);
  writeOctal(header, 116, 8, 0);
  writeOctal(header, 124, 12, data.byteLength);
  writeOctal(header, 136, 12, 0);
  header.fill(32, 148, 156);
  header[156] = '0'.charCodeAt(0);
  writeText(header, 257, 6, 'ustar');
  writeText(header, 263, 2, '00');
  writeChecksum(header);

  output.set(header, 0);
  output.set(data, blockSize);

  return output;
};

/** Extracts the first regular file from a small tar archive returned by Docker. */
export const extractFirstFile = (archive: Uint8Array): TarFile => {
  let offset = 0;

  while (offset + blockSize <= archive.byteLength) {
    const header = archive.subarray(offset, offset + blockSize);

    if (isEmpty(header)) {
      break;
    }

    const name = readText(header, 0, 100);
    const size = readOctal(header, 124, 12);
    const type = String.fromCharCode(header[156] ?? 0);
    const start = offset + blockSize;
    const end = start + size;

    if (end > archive.byteLength) {
      throw new Error(`Tar entry ${name} extends past archive end`);
    }

    if (type === '0' || type === '\0') {
      return { name, data: archive.slice(start, end) };
    }

    offset = start + size + padding(size);
  }

  throw new Error('Archive did not contain a regular file');
};

const padding = (size: number): number =>
  (blockSize - (size % blockSize)) % blockSize;

const writeText = (
  output: Uint8Array,
  offset: number,
  length: number,
  value: string,
): void => {
  output.set(Buffer.from(value).subarray(0, length), offset);
};

const writeOctal = (
  output: Uint8Array,
  offset: number,
  length: number,
  value: number,
): void => {
  const text = value.toString(8).padStart(length - 1, '0');

  writeText(output, offset, length - 1, text);
  output[offset + length - 1] = 0;
};

const writeChecksum = (header: Uint8Array): void => {
  const checksum = header.reduce((total, byte) => total + byte, 0);
  const text = checksum.toString(8).padStart(6, '0');

  writeText(header, 148, 6, text);
  header[154] = 0;
  header[155] = 32;
};

const readText = (
  input: Uint8Array,
  offset: number,
  length: number,
): string => {
  const raw = Buffer.from(input.subarray(offset, offset + length)).toString(
    'utf8',
  );

  return raw.replace(/\0.*$/u, '');
};

const readOctal = (
  input: Uint8Array,
  offset: number,
  length: number,
): number => {
  const raw = readText(input, offset, length).trim();

  return raw.length === 0 ? 0 : Number.parseInt(raw, 8);
};

const isEmpty = (input: Uint8Array): boolean =>
  input.every((byte) => byte === 0);
