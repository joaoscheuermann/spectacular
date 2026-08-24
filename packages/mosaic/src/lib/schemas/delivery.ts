import * as z from 'zod';

import { ArtifactSchema } from './artifact.js';
import { ObservationSchema } from './observation.js';

const FinalDeliveryPartSchema = z
  .object({
    id: z.string().trim().min(1),
    goal: z.string().trim().min(1),
    markdown: z.string().min(1),
    artifacts: z.array(ArtifactSchema),
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
