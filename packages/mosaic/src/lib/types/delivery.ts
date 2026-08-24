import type { Observation } from '../schemas/observation.js';
import type { Artifact } from './artifact.js';

/** One completed terminal node selected for the final response. */
export interface FinalDeliveryPart {
  readonly id: string;
  readonly goal: string;
  readonly markdown: string;
  readonly artifacts: readonly Artifact[];
  readonly observations: readonly Observation[];
}

/** The deterministic, model-free result of a completed MOSAIC workflow. */
export interface FinalDelivery {
  readonly markdown: string;
  readonly parts: readonly FinalDeliveryPart[];
}
