import { mkdir, rename, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { z } from 'zod';

import { schemasV1 } from './index.js';

const schemaDirectory = fileURLToPath(
  new URL('../../schemas/v1', import.meta.url),
);

/** Regenerates the checked-in JSON Schemas from the canonical Zod contracts. */
export const generateSchemas = async (
  outputDirectory = schemaDirectory,
): Promise<readonly string[]> => {
  await mkdir(outputDirectory, { recursive: true });

  return Promise.all(
    Object.entries(schemasV1).map(async ([name, schema]) => {
      const path = join(outputDirectory, `${name}.schema.json`);
      const temporary = `${path}.tmp`;
      const json = z.toJSONSchema(schema, {
        io: 'input',
        unrepresentable: 'any',
      });
      await mkdir(dirname(path), { recursive: true });
      await writeFile(temporary, `${JSON.stringify(json, null, 2)}\n`, {
        encoding: 'utf8',
        flag: 'w',
      });
      await rename(temporary, path);
      return path;
    }),
  );
};
