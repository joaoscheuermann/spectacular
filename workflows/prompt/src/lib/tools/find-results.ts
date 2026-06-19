import { posix as path } from 'node:path';

export type FindOutput = {
  readonly results: readonly string[];
  readonly total: number;
  readonly truncated: boolean;
  readonly error?: string;
};

const MAX_OUTPUT_BYTES = 50 * 1024;

export const collectFind = (
  searchPath: string,
  files: readonly string[],
  glob: RegExp,
  limit: number,
): FindOutput => {
  const results: string[] = [];
  let totalBytes = 0;
  let totalMatched = 0;
  let truncated = false;

  for (const file of files) {
    const relative = path.relative(searchPath, file);
    const name = path.basename(file);

    if (!glob.test(relative) && !glob.test(name)) {
      continue;
    }

    totalMatched += 1;
    const lineBytes = relative.length + 1;
    if (totalBytes + lineBytes > MAX_OUTPUT_BYTES || results.length >= limit) {
      truncated = true;
      break;
    }

    totalBytes += lineBytes;
    results.push(relative);
  }

  return {
    results,
    total: truncated ? totalMatched : results.length,
    truncated,
  };
};
