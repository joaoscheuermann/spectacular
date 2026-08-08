import { lstat, mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, isAbsolute, join, normalize, sep } from 'node:path';

import { artifactHash } from '../core/hash.js';

export interface PackageResult {
  readonly files: Readonly<Record<string, string>>;
  readonly packageHash: string;
}

const safeRelative = (value: string): string => {
  const normalized = normalize(value);
  if (
    isAbsolute(value) ||
    normalized === '..' ||
    normalized.startsWith(`..${sep}`)
  ) {
    throw new TypeError('package paths must remain below the study root');
  }
  return normalized;
};

/** Copies an explicit regular-file set into a non-overwriting reproducibility package. */
export const packageStudy = async (
  root: string,
  destination: string,
  relativePaths: readonly string[],
): Promise<PackageResult> => {
  const unique = [...new Set(relativePaths.map(safeRelative))].sort();
  const entries = await Promise.all(
    unique.map(async (relativePath) => {
      const source = join(root, relativePath);
      const metadata = await lstat(source);
      if (!metadata.isFile() || metadata.isSymbolicLink())
        throw new TypeError(
          `package source is not a regular file: ${relativePath}`,
        );
      const content = await readFile(source);
      const target = join(destination, relativePath);
      await mkdir(dirname(target), { recursive: true });
      await writeFile(target, content, { flag: 'wx', mode: 0o600 });
      return [relativePath, artifactHash(content.toString('base64'))] as const;
    }),
  );
  const files = Object.fromEntries(entries);
  return { files, packageHash: artifactHash(files) };
};
