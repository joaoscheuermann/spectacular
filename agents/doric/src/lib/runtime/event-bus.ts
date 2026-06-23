import {
  DefaultExecutionEventBusManager,
  type ExecutionEventBus,
  type ExecutionEventBusManager,
  type ExecutionEventName,
} from '@a2a-js/sdk/server';

import type { RuntimeStore } from './store.js';

/** Records Doric runtime events while preserving the SDK event bus contract. */
export const createRecordingEventBusManager = (
  runtime: RuntimeStore,
  delegate: ExecutionEventBusManager = new DefaultExecutionEventBusManager(),
): ExecutionEventBusManager => {
  const buses = new Map<string, ExecutionEventBus>();

  return {
    createOrGetByTaskId(taskId) {
      const existing = buses.get(taskId);

      if (existing !== undefined) {
        return existing;
      }

      const bus = recordingBus(delegate.createOrGetByTaskId(taskId), runtime);
      buses.set(taskId, bus);

      return bus;
    },

    getByTaskId(taskId) {
      return buses.get(taskId);
    },

    cleanupByTaskId(taskId) {
      buses.delete(taskId);
      delegate.cleanupByTaskId(taskId);
    },
  };
};

const recordingBus = (
  delegate: ExecutionEventBus,
  runtime: RuntimeStore,
): ExecutionEventBus => ({
  publish(event) {
    runtime.recordEvent(event);
    delegate.publish(event);
  },

  on(eventName, listener) {
    delegate.on(eventName, listener);

    return this;
  },

  off(eventName, listener) {
    delegate.off(eventName, listener);

    return this;
  },

  once(eventName, listener) {
    delegate.once(eventName, listener);

    return this;
  },

  removeAllListeners(eventName?: ExecutionEventName) {
    delegate.removeAllListeners(eventName);

    return this;
  },

  finished() {
    delegate.finished();
  },
} satisfies ExecutionEventBus);
