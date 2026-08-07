import * as z from 'zod';

import { NodeArtifactsSchema } from './graph.js';
import { ObservationSchema } from './revision.js';

const FinalDeliveryPartSchema = z
  .object({
    id: z.string().trim().min(1),
    goal: z.string().trim().min(1),
    markdown: z.string().min(1),
    artifacts: z.array(NodeArtifactsSchema),
    observations: z.array(ObservationSchema),
  })
  .strict();

/** Strict public contract returned after deterministic final assembly. */
export const FinalDeliverySchema = z
  .object({
    markdown: z.string().min(1),
    parts: z.array(FinalDeliveryPartSchema).min(1),
  })
  .strict();
