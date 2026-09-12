import type { MosaicRunOptions } from './events.js';
import type { MosaicResult } from './result.js';

export interface MosaicAgent {
  prompt(input: string, options?: MosaicRunOptions): Promise<MosaicResult>;
}
