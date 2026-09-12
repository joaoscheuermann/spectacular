import { stat } from 'node:fs/promises';
import { resolve } from 'node:path';

/** Finds the benchmark project from either the workspace or project directory. */
export const benchmarkRoot = async (cwd: string): Promise<string> => {
  const candidates = [resolve(cwd, 'benchmarks', 'mosaic'), resolve(cwd)];

  for (const candidate of candidates) {
    try {
      if ((await stat(resolve(candidate, 'project.json'))).isFile())
        {return candidate;}
    } catch {
      // Try the next supported invocation directory.
    }
  }

  throw new Error('Cannot find the mosaic-benchmark project root.');
};
