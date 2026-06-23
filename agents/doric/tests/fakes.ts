import { Buffer } from 'node:buffer';
import { randomUUID } from 'node:crypto';

import type { Message } from '@a2a-js/sdk';
import {
  RequestContext,
  type AgentExecutionEvent,
  type AgentExecutor,
  type ExecutionEventBus,
  type ExecutionEventName,
} from '@a2a-js/sdk/server';
import type { LlmProvider } from 'llms';
import type { AgentConfig, ProviderConfig } from 'config';
import type { DockerClient } from 'docker';
import { createSessionStore, type SessionStore } from 'session';
import type {
  CloneRepoInput,
  ClonedRepo,
  CreateSandboxOptions,
  SandboxExecInput,
  SandboxExecResult,
  SandboxSession,
} from 'sandbox';
import type { PromptArtifact, PromptWorkflowOptions } from 'workflow-prompt';

import {
  createExecutor,
  type DoricSessionContext,
  type DoricExecutorLogger,
  type ProviderFactory,
  type PromptRunner,
} from '../src/lib/executor.js';

export type FakeSandbox = SandboxSession & {
  readonly execs: SandboxExecInput[];
  readonly clones: CloneRepoInput[];
  disposed: boolean;
};

export type DoricTestHarness = {
  readonly docker: DockerClient;
  readonly executor: AgentExecutor;
  readonly sandboxes: FakeSandbox[];
  readonly sandboxOptions: CreateSandboxOptions[];
  readonly promptRuns: PromptRun[];
  readonly providerConfigs: ProviderConfig[];
  readonly dockerCreateCount: () => number;
};

export type CapturingEventBus = ExecutionEventBus & {
  readonly events: AgentExecutionEvent[];
  finishedCount: number;
};

export type CapturingLogger = DoricExecutorLogger & {
  readonly entries: CapturingLogEntry[];
};

export type CapturingLogEntry = {
  readonly level: 'info' | 'error';
  readonly bindings: Record<string, unknown>;
  readonly message: string;
};

type HarnessOptions = {
  readonly sessions?: SessionStore<DoricSessionContext>;
  readonly hasGit?: boolean;
  readonly gitInstallFailure?: Error;
  readonly cloneFailure?: Error;
  readonly logger?: DoricExecutorLogger;
  readonly promptRunner?: PromptRunner;
  readonly createProvider?: ProviderFactory;
  readonly useDefaultProvider?: boolean;
};

export type PromptRun = {
  readonly artifact: PromptArtifact;
  readonly options: PromptWorkflowOptions;
};

const GIT_PROBE_COMMAND = 'sh -lc command -v git >/dev/null 2>&1';
const GIT_INSTALL_COMMAND =
  'sh -lc apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*';

export const createDoricTestHarness = (
  options: HarnessOptions = {},
): DoricTestHarness => {
  const docker = {} as DockerClient;
  const sandboxes: FakeSandbox[] = [];
  const sandboxOptions: CreateSandboxOptions[] = [];
  const promptRuns: PromptRun[] = [];
  const providerConfigs: ProviderConfig[] = [];
  const sessions =
    options.sessions ?? createSessionStore<DoricSessionContext>();
  let dockerCreateCount = 0;

  return {
    docker,
    sandboxes,
    sandboxOptions,
    promptRuns,
    providerConfigs,
    dockerCreateCount: () => dockerCreateCount,
    executor: createExecutor({
      sessions,
      createDockerClient: () => {
        dockerCreateCount += 1;

        return docker;
      },
      createSandbox: async (input: CreateSandboxOptions) => {
        sandboxOptions.push(input);

        const sandbox = createFakeSandbox({
          hasGit: options.hasGit ?? true,
          gitInstallFailure: options.gitInstallFailure,
          cloneFailure: options.cloneFailure,
        });

        sandboxes.push(sandbox);

        return sandbox;
      },
      logger: options.logger,
      ...(options.useDefaultProvider
        ? {}
        : {
            createProvider:
              options.createProvider ??
              ((provider: ProviderConfig) => {
                providerConfigs.push(provider);

                return createFakeProvider(provider);
              }),
          }),
      promptRunner: async (artifact, input) => {
        promptRuns.push({ artifact, options: input });

        return options.promptRunner?.(artifact, input) ?? artifact;
      },
    }),
  };
};

export const createLogger = (): CapturingLogger => {
  const entries: CapturingLogEntry[] = [];

  return {
    entries,
    info(bindings, message) {
      entries.push({ level: 'info', bindings, message });
    },
    error(bindings, message) {
      entries.push({ level: 'error', bindings, message });
    },
  };
};

export const createEventBus = (): CapturingEventBus => {
  const bus: CapturingEventBus = {
    events: [],
    finishedCount: 0,
    publish(event) {
      bus.events.push(event);
    },
    on(_eventName: ExecutionEventName, _listener) {
      return bus;
    },
    off(_eventName: ExecutionEventName, _listener) {
      return bus;
    },
    once(_eventName: ExecutionEventName, _listener) {
      return bus;
    },
    removeAllListeners(_eventName?: ExecutionEventName) {
      return bus;
    },
    finished() {
      bus.finishedCount += 1;
    },
  };

  return bus;
};

export const createRequestContext = (
  options: {
    readonly contextId?: string;
    readonly config?: AgentConfig;
    readonly metadata?: Message['metadata'] | null;
    readonly message?: Message;
    readonly parts?: Message['parts'];
  } = {},
): RequestContext => {
  const contextId = options.contextId ?? randomUUID();
  const metadata =
    options.metadata === null
      ? undefined
      : (options.metadata ?? {
          configuration: options.config ?? createConfig(),
        });
  const message =
    options.message ??
    createUserMessage({
      contextId,
      metadata,
      parts: options.parts ?? [createTextPart()],
    });

  return new RequestContext(message, randomUUID(), contextId);
};

export const createUserMessage = (options: {
  readonly contextId?: string;
  readonly metadata?: Message['metadata'];
  readonly parts: Message['parts'];
}): Message => ({
  kind: 'message',
  messageId: randomUUID(),
  role: 'user',
  ...(options.contextId === undefined ? {} : { contextId: options.contextId }),
  ...(options.metadata === undefined ? {} : { metadata: options.metadata }),
  parts: options.parts,
});

export const createTextPart = (
  text = 'Build the agent',
): Message['parts'][number] => ({
  kind: 'text',
  text,
});

export const createConfig = (
  options: {
    readonly repoUrl?: string;
    readonly token?: string;
    readonly providers?: AgentConfig['providers'];
    readonly models?: AgentConfig['models'];
    readonly tasks?: AgentConfig['tasks'];
  } = {},
): AgentConfig => ({
  github: {
    repo: {
      url: options.repoUrl ?? 'https://github.com/example/repo',
    },
    token: options.token ?? 'github-token',
  },
  providers: [
    ...(options.providers ?? [
      {
        id: 'openai',
        type: 'openai',
        token: 'provider-token',
      },
    ]),
  ],
  models: [
    ...(options.models ?? [
      {
        id: 'default',
        provider: 'openai',
        model: 'gpt-5',
      },
    ]),
  ],
  tasks: [
    ...(options.tasks ?? [
      {
        id: 'coding',
        model: 'default',
      },
    ]),
  ],
});

export const ok = (stdout = ''): SandboxExecResult => ({
  exitCode: 0,
  stdout,
  stderr: '',
  stdoutBytes: Buffer.from(stdout),
  stderrBytes: new Uint8Array(),
});

const failed = (stderr = ''): SandboxExecResult => ({
  exitCode: 1,
  stdout: '',
  stderr,
  stdoutBytes: new Uint8Array(),
  stderrBytes: Buffer.from(stderr),
});

const createFakeSandbox = (options: {
  readonly hasGit: boolean;
  readonly gitInstallFailure: Error | undefined;
  readonly cloneFailure: Error | undefined;
}): FakeSandbox => {
  const execs: SandboxExecInput[] = [];
  const clones: CloneRepoInput[] = [];
  let gitInstalled = options.hasGit;

  return {
    id: randomUUID(),
    root: '/workspace',
    execs,
    clones,
    disposed: false,

    async exec(input) {
      execs.push(input);

      if (input.cmd.join(' ') === GIT_PROBE_COMMAND) {
        return gitInstalled ? ok() : failed('git missing');
      }

      if (input.cmd.join(' ') === GIT_INSTALL_COMMAND) {
        if (options.gitInstallFailure !== undefined) {
          return failed(options.gitInstallFailure.message);
        }

        gitInstalled = true;

        return ok();
      }

      return ok();
    },

    async cloneRepo(input): Promise<ClonedRepo> {
      clones.push(input);

      if (options.cloneFailure !== undefined) {
        throw options.cloneFailure;
      }

      return { path: '/workspace/repo', commit: 'abc123' };
    },

    async readFile() {
      return '';
    },

    async writeFile() {
      return undefined;
    },

    async putFile() {
      return undefined;
    },

    async getFile() {
      return new Uint8Array();
    },

    async diff() {
      return '';
    },

    async dispose() {
      this.disposed = true;
    },
  };
};

const createFakeProvider = (provider: ProviderConfig): LlmProvider =>
  ({
    metadata: {
      id: provider.id,
      name: provider.id,
      baseUrl: 'https://example.invalid',
    },
    capabilities: {
      streaming: true,
      tools: true,
      reasoning: true,
      modelListing: false,
      oauth: false,
      serviceTier: false,
      structuredOutputs: true,
    },
    async complete() {
      throw new Error('Fake provider complete was not expected.');
    },
    async *stream() {
      throw new Error('Fake provider stream was not expected.');
    },
    async models() {
      return [];
    },
    async validateModel(model: string) {
      return { id: model };
    },
  }) as LlmProvider;
