import type { Logger } from 'pino';
import type { SandboxSshAccess } from 'sandbox';
import type { PooledSandbox, SandboxLease, Sandpool } from 'sandpool';
import type { Tool, ToolFactory } from 'tool';
import type { Search } from 'victor';
import createMosaic, { type MosaicEvent } from 'mosaic';

import type { ConfigService } from './config-service.js';
import { providerFor, type Generation } from './generation.js';
import type { SessionPublisher } from './session-publisher.js';
import type { Session, SessionStore } from './sessions.js';

export type SessionService = ReturnType<typeof createSessionService>;

type SessionServiceOptions = {
  readonly store: SessionStore;
  readonly config: ConfigService;
  readonly pool: Sandpool;
  readonly publisher: SessionPublisher;
  readonly logger: Logger;
  readonly execute?: SessionExecution;
};

type LiveSandbox =
  | { readonly status: 'pending' }
  | { readonly status: 'ready'; readonly sandbox: PooledSandbox }
  | { readonly status: 'unavailable' };

export type SessionSsh =
  | { readonly status: 'pending' }
  | { readonly status: 'unavailable' }
  | { readonly status: 'expired' }
  | { readonly status: 'missing' }
  | {
      readonly status: 'ready';
      readonly vmId: string;
      readonly ssh: SandboxSshAccess;
    };

export type SessionEvents = {
  readonly events: readonly unknown[];
  readonly lastSequence: number;
};

/** Queues one isolated sandbox-backed Mosaic execution per persisted session. */
export const createSessionService = ({
  store,
  config,
  pool,
  publisher,
  logger,
  execute: executeSession = executeMosaic,
}: SessionServiceOptions) => {
  const controllers = new Map<string, AbortController>();
  const running = new Set<Promise<void>>();
  const live = new Map<string, LiveSandbox>();
  const sessionsByVm = new Map<string, string>();

  const execute = (id: string, prompt: string, generation: Generation) => {
    const controller = new AbortController();
    controllers.set(id, controller);
    const execution = run(
      {
        id,
        prompt,
        generation,
        signal: controller.signal,
        store,
        pool,
        publisher,
        logger,
        acquired: (sandbox) => {
          live.set(id, { status: 'ready', sandbox });
          sessionsByVm.set(sandbox.id, id);
        },
        releasing: (sandbox) => {
          sessionsByVm.delete(sandbox.id);
          live.set(id, { status: 'unavailable' });
        },
      },
      executeSession,
    )
      .catch(() =>
        logger.error({ sessionId: id }, 'Mosaic session persistence failed'),
      )
      .finally(() => {
        controllers.delete(id);
        live.delete(id);
        running.delete(execution);
      });
    running.add(execution);
  };

  return {
    async create(prompt: string): Promise<Session> {
      const generation = config.current();
      const record = await store.create(prompt, generation.snapshot);
      live.set(record.session.id, { status: 'pending' });
      execute(record.session.id, prompt, generation);
      return record.session;
    },

    list: store.list,

    ssh: (id: string) => findSessionSsh(id, live, store),

    sshForVm: (id: string) => findVmSsh(id, sessionsByVm, live),

    events: (id: string, afterSequence: number) =>
      findEvents(id, afterSequence, store),

    async terminate(id: string): Promise<Session | undefined> {
      const current = await store.requestCancellation(id);
      if (current === undefined) return undefined;
      controllers.get(id)?.abort();
      publisher.updated(current);
      return current;
    },

    async delete(id: string) {
      const outcome = await store.delete(id);
      if (outcome === 'deleted') publisher.deleted(id);
      return outcome;
    },

    async dispose(): Promise<void> {
      controllers.forEach((controller) => controller.abort());
      await Promise.allSettled([...running]);
    },
  };
};

type RunOptions = {
  readonly id: string;
  readonly prompt: string;
  readonly generation: Generation;
  readonly signal: AbortSignal;
  readonly store: SessionStore;
  readonly pool: Sandpool;
  readonly publisher: SessionPublisher;
  readonly logger: Logger;
  readonly acquired: (sandbox: PooledSandbox) => void;
  readonly releasing: (sandbox: PooledSandbox) => void;
};

type SessionExecution = (
  options: RunOptions,
  lease: SandboxLease,
) => Promise<unknown>;

const run = async (
  options: RunOptions,
  executeSession: SessionExecution,
): Promise<void> => {
  const { id, store, publisher, signal, logger, pool } = options;
  try {
    const lease = await pool.acquire({ signal });
    options.acquired(lease.sandbox);
    let result: unknown;
    try {
      signal.throwIfAborted();
      const started = await store.markRunning(id);
      if (started === undefined) return;
      publisher.updated(started);
      if (started.state === 'cancelling') {
        await finish(options, 'cancelled');
        return;
      }
      result = await executeSession(options, lease);
    } finally {
      options.releasing(lease.sandbox);
      await lease.release();
    }
    await finish(options, 'completed', result);
  } catch (error) {
    const cancelled = signal.aborted || isAbort(error);
    await finish(
      options,
      cancelled ? 'cancelled' : 'failed',
      undefined,
      cancelled ? undefined : 'execution_failed',
    );
    logger.debug(
      { sessionId: id, status: cancelled ? 'cancelled' : 'failed' },
      'Mosaic session finished',
    );
  }
};

const executeMosaic = async (
  { id, prompt, generation, signal, store, publisher, logger }: RunOptions,
  lease: SandboxLease,
) => {
  const tools = generation.catalog.tools.map(
    ({ factory, alwaysAvailable }) => ({
      tool: factory(lease.sandbox),
      alwaysAvailable,
    }),
  );
  const agent = createMosaic({
    logger,
    providers: providers(generation),
    models: models(generation),
    routing: generation.snapshot.configuration.routing,
    execution: generation.snapshot.configuration.execution,
    revision: generation.snapshot.configuration.revision,
    skills: skillOptions(generation),
    tools: toolOptions(generation, tools),
  });
  return await agent.prompt(prompt, {
    runId: id,
    signal,
    capture: 'io',
    observer: async (event: MosaicEvent) => {
      const stored = await store.appendEvent(id, event);
      publisher.event(stored);
    },
  });
};

const providers = (generation: Generation) => {
  const { models } = generation.snapshot.configuration;
  return {
    planning: providerFor(generation, models.planning.providerId),
    revision: providerFor(generation, models.revision.providerId),
    execution: providerFor(generation, models.execution.providerId),
    reranker: providerFor(generation, models.reranker.providerId),
  };
};

const models = (generation: Generation) => {
  const { models } = generation.snapshot.configuration;
  return {
    planning: { model: models.planning.model, effort: models.planning.effort },
    revision: { model: models.revision.model, effort: models.revision.effort },
    execution: {
      model: models.execution.model,
      effort: models.execution.effort,
    },
    reranker: models.reranker.model,
    embedder: models.embedder.model,
  };
};

const skillOptions = (generation: Generation) => {
  const menu = generation.catalog.skills.map(({ skill }) => skill);
  return {
    required: generation.catalog.skills
      .filter(({ alwaysAvailable }) => alwaysAvailable)
      .map(({ skill }) => skill),
    menu,
    retriever: generation.retrieval.skills,
  };
};

const toolOptions = (
  generation: Generation,
  entries: readonly {
    readonly tool: Tool;
    readonly alwaysAvailable: boolean;
  }[],
) => {
  const menu = entries.map(({ tool }) => tool);
  const byName = new Map(menu.map((tool) => [tool.name, tool]));
  const retriever: Search<Tool> = {
    search: async (query, topK) =>
      (await generation.retrieval.tools.search(query, topK)).flatMap(
        ({ data, score }) => {
          const tool = byName.get((data as ToolFactory).name);
          return tool === undefined ? [] : [{ data: tool, score }];
        },
      ),
  };
  return {
    required: entries
      .filter(({ alwaysAvailable }) => alwaysAvailable)
      .map(({ tool }) => tool),
    menu,
    retriever,
  };
};

const finish = async (
  { id, store, publisher }: RunOptions,
  state: 'completed' | 'failed' | 'cancelled',
  result?: unknown,
  errorCode?: string,
) => {
  const current = await store.finish(id, state, result, errorCode);
  if (current !== undefined) publisher.updated(current);
};

const isAbort = (error: unknown): boolean =>
  error instanceof Error && error.name === 'AbortError';

const terminal = new Set<Session['state']>([
  'completed',
  'failed',
  'cancelled',
]);

const findSessionSsh = async (
  id: string,
  live: ReadonlyMap<string, LiveSandbox>,
  store: SessionStore,
): Promise<SessionSsh> => {
  const current = live.get(id);
  if (current?.status === 'pending') return { status: 'pending' };
  if (current?.status === 'unavailable') return { status: 'unavailable' };
  if (current?.status === 'ready') {
    const ssh = await current.sandbox.ssh();
    return ssh === undefined
      ? { status: 'unavailable' }
      : { status: 'ready', vmId: current.sandbox.id, ssh };
  }

  const record = await store.find(id);
  if (record === undefined) return { status: 'missing' };
  return {
    status: terminal.has(record.session.state) ? 'expired' : 'unavailable',
  };
};

const findVmSsh = async (
  id: string,
  sessionsByVm: ReadonlyMap<string, string>,
  live: ReadonlyMap<string, LiveSandbox>,
): Promise<
  { readonly sessionId: string; readonly ssh: SandboxSshAccess } | undefined
> => {
  const sessionId = sessionsByVm.get(id);
  if (sessionId === undefined) return undefined;
  const current = live.get(sessionId);
  if (current?.status !== 'ready' || current.sandbox.id !== id)
    return undefined;
  const ssh = await current.sandbox.ssh();
  return ssh === undefined ? undefined : { sessionId, ssh };
};

const findEvents = async (
  id: string,
  afterSequence: number,
  store: SessionStore,
): Promise<SessionEvents | undefined> => {
  const record = await store.find(id);
  if (record === undefined) return undefined;
  const events = await store.eventsAfter(id, afterSequence);
  return {
    events: events.map(({ event }) => event),
    lastSequence: events.at(-1)?.sequence ?? record.session.lastSequence,
  };
};
