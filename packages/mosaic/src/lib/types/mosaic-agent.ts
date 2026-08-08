import type { MosaicResult } from './result.js';
import type { MosaicRunOptions } from './events.js';

export interface MosaicAgent {
  prompt(input: string, options?: MosaicRunOptions): Promise<MosaicResult>;
}
