import type { MosaicAgent } from './types/mosaic-agent.js';
import type { MosaicOptions } from './types/mosaic-options.js';
import { validateOptions } from './options.js';
import { createMachine } from './workflow/machine.js';

export function mosaic(options: MosaicOptions): MosaicAgent {
  validateOptions(options);
  const machine = createMachine();

  return {
    async prompt(input: string): Promise<void> {
      const result = await machine.run({
        initial: 'graph',
        state: { graphs: [] },
        context: { input, options },
      });

      if (result.status === 'finished') {
        return;
      }

      throw result.error;
    },
  };
}

export default mosaic;
