import type { FinalDelivery } from './delivery.js';

export interface MosaicAgent {
  prompt(input: string): Promise<FinalDelivery>;
}
