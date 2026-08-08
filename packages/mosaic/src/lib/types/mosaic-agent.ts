import type { MosaicResult } from './result.js';

export interface MosaicAgent {
  prompt(input: string): Promise<MosaicResult>;
}
