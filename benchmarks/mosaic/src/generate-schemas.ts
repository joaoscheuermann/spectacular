import { resolve } from 'node:path';

import { generateSchemas } from './schemas/generate.js';

const paths = await generateSchemas(
  resolve(process.argv[2] ?? 'benchmarks/mosaic/schemas/v1'),
);
process.stdout.write(`${JSON.stringify({ generated: paths.length })}\n`);
