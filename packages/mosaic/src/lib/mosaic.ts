import type { MosaicAgent } from './types/mosaic-agent.js';
import type { MosaicResult } from './types/result.js';
import type { MosaicOptions } from './types/mosaic-options.js';
import { validateOptions } from './options.js';
import { createMachine } from './workflow/machine.js';
import { createRuntime } from './observability.js';
import type { MosaicRunOptions } from './types/events.js';
import type { MosaicEvaluationHooks } from './types/evaluation.js';
import { createObservationIdAllocator } from './observation-ids.js';

export function mosaic(options: MosaicOptions): MosaicAgent {
  return createMosaic(options);
}

/** Internal shared engine used by the public and evaluation entrypoints. */
export function createMosaic(
  options: MosaicOptions,
  hooks?: MosaicEvaluationHooks,
): MosaicAgent {
  validateOptions(options);
  const machine = createMachine();

  return {
    async prompt(
      input: string,
      runOptions: MosaicRunOptions = {},
    ): Promise<MosaicResult> {
      const runtime = createRuntime(runOptions);
      const timer = runtime.timer();

      try {
        await runtime.emit({
          type: 'run.started',
          stage: 'run',
          ...(runtime.capture === 'io' ? { input } : {}),
        });

        const result = await machine.run({
          initial: 'plan',
          state: { graphs: [] },
          context: {
            input,
            options,
            runtime,
            observationIds: createObservationIdAllocator(),
            ...(hooks === undefined ? {} : { hooks }),
          },
        });

        if (runtime.hasFailed) throw runtime.failure;
        if (result.status !== 'finished') throw result.error;
        if (result.value === undefined) {
          throw new Error('Workflow finished without a terminal result.');
        }

        await runtime.emit({
          type: 'run.finished',
          stage: 'run',
          durationMs: runtime.duration(timer),
          status: result.value.status,
        });
        return result.value;
      } catch (error) {
        if (runtime.hasFailed) throw runtime.failure;
        await runtime.emit({
          type: 'run.failed',
          stage: 'run',
          durationMs: runtime.duration(timer),
        });
        throw error;
      }
    },
  };
}

export default mosaic;
