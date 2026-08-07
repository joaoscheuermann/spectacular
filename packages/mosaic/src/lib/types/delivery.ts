import type { Observation } from './revision.js';

/** A concrete artifact included with a completed deliverable. */
export interface DeliveryArtifact {
  readonly mime: string;
  readonly data: string;
}

/** One completed terminal node selected for the final response. */
export interface FinalDeliveryPart {
  readonly id: string;
  readonly goal: string;
  readonly markdown: string;
  readonly artifacts: readonly DeliveryArtifact[];
  readonly observations: readonly Observation[];
}

/** The deterministic, model-free result of a completed MOSAIC workflow. */
export interface FinalDelivery {
  readonly markdown: string;
  readonly parts: readonly FinalDeliveryPart[];
}
