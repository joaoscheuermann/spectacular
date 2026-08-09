import { randomUUID } from 'node:crypto';
import { performance } from 'node:perf_hooks';

import type {
  JsonValue,
  LlmProvider,
  ProviderFinished,
  ProviderRequest,
  ProviderRerankRequest,
} from 'llms';

import type {
  MosaicCapture,
  MosaicEvent,
  MosaicRunOptions,
  MosaicStage,
} from './types/events.js';

type WithoutBase<Event> = Event extends MosaicEvent
  ? Omit<Event, 'schemaVersion' | 'runId' | 'sequence'>
  : never;

export type MosaicEventInput = WithoutBase<MosaicEvent>;

export type Timer = {
  readonly startedAt: number;
  readonly observedAtStart: number;
};

export interface MosaicRuntime {
  readonly capture: MosaicCapture;
  readonly signal?: AbortSignal;
  readonly hasFailed: boolean;
  readonly failure: unknown;
  emit(event: MosaicEventInput): Promise<void>;
  timer(): Timer;
  duration(timer: Timer): number;
  provider(
    provider: LlmProvider,
    stage: MosaicStage,
    nodeId?: string,
    revision?: number,
  ): LlmProvider;
}

/** Creates one isolated, serial observer boundary for a MOSAIC prompt. */
export const createRuntime = (
  options: MosaicRunOptions = {},
): MosaicRuntime => {
  const capture = options.capture ?? 'structure';
  if (capture !== 'structure' && capture !== 'io') {
    throw new TypeError('Mosaic capture must be structure or io.');
  }

  const runId = options.runId ?? randomUUID();
  if (!uuid.test(runId)) {
    throw new TypeError('Mosaic runId must be a UUID.');
  }
  const observer = options.observer;
  let sequence = 0;
  let observedMs = 0;
  let failed = false;
  let failure: unknown;
  let queue = Promise.resolve();

  const emit = async (input: MosaicEventInput): Promise<void> => {
    options.signal?.throwIfAborted();
    if (failed) throw failure;
    if (observer === undefined) return;

    const event = immutable({
      schemaVersion: 2 as const,
      runId,
      sequence: ++sequence,
      ...input,
    } as MosaicEvent);
    const delivery = queue.then(async () => {
      if (failed) throw failure;
      const startedAt = performance.now();
      try {
        await observer(event);
      } catch (error) {
        failed = true;
        failure = error;
        throw error;
      } finally {
        observedMs += performance.now() - startedAt;
      }
    });
    queue = delivery.catch(() => undefined);
    await delivery;
  };

  const runtime: MosaicRuntime = {
    capture,
    signal: options.signal,
    get hasFailed() {
      return failed;
    },
    get failure() {
      return failed ? failure : undefined;
    },
    emit,
    timer: () => ({
      startedAt: performance.now(),
      observedAtStart: observedMs,
    }),
    duration: (timer) =>
      Math.max(
        0,
        performance.now() -
          timer.startedAt -
          (observedMs - timer.observedAtStart),
      ),
    provider: (provider, stage, nodeId, revision) =>
      observedProvider(provider, runtime, stage, nodeId, revision),
  };

  return runtime;
};

const uuid =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/iu;

const observedProvider = (
  provider: LlmProvider,
  runtime: MosaicRuntime,
  stage: MosaicStage,
  nodeId?: string,
  revision?: number,
): LlmProvider =>
  new Proxy(provider, {
    get(target, property, receiver) {
      if (property === 'complete')
        return complete(target, runtime, stage, nodeId, revision);
      if (property === 'stream')
        return stream(target, runtime, stage, nodeId, revision);
      if (property === 'rerank')
        return rerank(target, runtime, stage, nodeId, revision);
      const value = Reflect.get(target, property, receiver) as unknown;
      return typeof value === 'function' ? value.bind(target) : value;
    },
  });

const complete =
  (
    provider: LlmProvider,
    runtime: MosaicRuntime,
    stage: MosaicStage,
    nodeId?: string,
    revision?: number,
  ) =>
  async <Output = JsonValue>(
    request: ProviderRequest<Output>,
  ): Promise<ProviderFinished<Output>> => {
    const current = withSignal(request, runtime.signal);
    await requestEvent(
      runtime,
      provider.metadata.id,
      current,
      'complete',
      stage,
      nodeId,
      revision,
    );
    const timer = runtime.timer();
    const response = await provider.complete(current);
    await responseEvent(
      runtime,
      provider.metadata.id,
      request.model,
      response,
      'complete',
      stage,
      timer,
      nodeId,
      revision,
    );
    return response;
  };

const stream = (
  provider: LlmProvider,
  runtime: MosaicRuntime,
  stage: MosaicStage,
  nodeId?: string,
  revision?: number,
) =>
  async function* <Output = JsonValue>(request: ProviderRequest<Output>) {
    const current = withSignal(request, runtime.signal);
    await requestEvent(
      runtime,
      provider.metadata.id,
      current,
      'stream',
      stage,
      nodeId,
      revision,
    );
    const timer = runtime.timer();
    for await (const event of provider.stream(current)) {
      if (event.type === 'response.finished') {
        await responseEvent(
          runtime,
          provider.metadata.id,
          request.model,
          event.finish,
          'stream',
          stage,
          timer,
          nodeId,
          revision,
        );
      }
      yield event;
    }
  };

const rerank =
  (
    provider: LlmProvider,
    runtime: MosaicRuntime,
    stage: MosaicStage,
    nodeId?: string,
    revision?: number,
  ) =>
  async (request: ProviderRerankRequest) => {
    const current = withSignal(request, runtime.signal);
    await runtime.emit({
      type: 'model.request',
      providerId: provider.metadata.id,
      stage,
      operation: 'rerank',
      model: current.model,
      ...(nodeId === undefined ? {} : { nodeId }),
      ...(revision === undefined ? {} : { revision }),
      ...(runtime.capture === 'io'
        ? { content: { query: current.query, documents: current.documents } }
        : {}),
    });
    const timer = runtime.timer();
    const response = await provider.rerank(current);
    await runtime.emit({
      type: 'model.response',
      providerId: provider.metadata.id,
      stage,
      operation: 'rerank',
      model: request.model,
      durationMs: runtime.duration(timer),
      ...(nodeId === undefined ? {} : { nodeId }),
      ...(revision === undefined ? {} : { revision }),
      ...(runtime.capture === 'io' ? { content: response } : {}),
    });
    return response;
  };

const requestEvent = async (
  runtime: MosaicRuntime,
  providerId: string,
  request: ProviderRequest,
  operation: 'complete' | 'stream',
  stage: MosaicStage,
  nodeId?: string,
  revision?: number,
): Promise<void> => {
  await runtime.emit({
    type: 'model.request',
    providerId,
    stage,
    operation,
    model: request.model,
    messageCount: request.messages.length,
    toolNames: request.tools?.map(({ name }) => name) ?? [],
    ...(nodeId === undefined ? {} : { nodeId }),
    ...(revision === undefined ? {} : { revision }),
    ...(runtime.capture === 'io' ? { content: visibleRequest(request) } : {}),
  });
};

const responseEvent = async (
  runtime: MosaicRuntime,
  providerId: string,
  model: string,
  response: ProviderFinished<unknown>,
  operation: 'complete' | 'stream',
  stage: MosaicStage,
  timer: Timer,
  nodeId?: string,
  revision?: number,
): Promise<void> => {
  await runtime.emit({
    type: 'model.response',
    providerId,
    stage,
    operation,
    model,
    finishReason: response.finishReason,
    durationMs: runtime.duration(timer),
    ...(nodeId === undefined ? {} : { nodeId }),
    ...(revision === undefined ? {} : { revision }),
    ...(runtime.capture === 'io' ? { content: visibleResponse(response) } : {}),
  });
};

const withSignal = <Request extends { readonly signal?: AbortSignal }>(
  request: Request,
  signal: AbortSignal | undefined,
): Request =>
  signal === undefined || request.signal !== undefined
    ? request
    : ({ ...request, signal } as Request);

const visibleRequest = (request: ProviderRequest): unknown => ({
  messages: request.messages,
  tools: request.tools,
});

const visibleResponse = (response: ProviderFinished<unknown>): unknown => ({
  text: response.text,
  finishReason: response.finishReason,
  refusal: response.refusal,
  toolCalls: response.toolCalls,
  structured: response.structured,
});

const immutable = <Value>(value: Value): Value =>
  freeze(structuredClone(value));

const freeze = <Value>(value: Value): Value => {
  if (typeof value !== 'object' || value === null || Object.isFrozen(value)) {
    return value;
  }
  for (const nested of Object.values(value)) freeze(nested);
  return Object.freeze(value);
};
