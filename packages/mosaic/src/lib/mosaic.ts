import type { MosaicAgent } from './types/mosaic-agent.js';
import type { MosaicResult } from './types/result.js';
import type { MosaicOptions } from './types/mosaic-options.js';
import { validateOptions } from './options.js';
import { createMachine } from './workflow/machine.js';

export function mosaic(options: MosaicOptions): MosaicAgent {
  validateOptions(options);
  const machine = createMachine();

  return {
    async prompt(input: string): Promise<MosaicResult> {
      const result = await machine.run({
        initial: 'plan',
        state: { graphs: [] },
        context: { input, options },
      });

      if (result.status === 'finished') {
        if (result.value !== undefined) return result.value;
        throw new Error('Workflow finished without a terminal result.');
      }

      throw result.error;
    },
  };
}

export default mosaic;
