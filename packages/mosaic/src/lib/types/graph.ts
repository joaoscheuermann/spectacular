import type * as z from 'zod';

import { NodeSchema, GraphSchema } from '../schemas/graph.js';

export type Node = z.output<typeof NodeSchema>;
export type Graph = z.output<typeof GraphSchema>;
