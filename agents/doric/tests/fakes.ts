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
import type { AgentConfig } from 'config';
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

import { createExecutor, type DoricSessionContext } from '../src/index.js';

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
  readonly dockerCreateCount: () => number;
};

export type CapturingEventBus = ExecutionEventBus & {
  readonly events: AgentExecutionEvent[];
  finishedCount: number;
};

type HarnessOptions = {
  readonly sessions?: SessionStore<DoricSessionContext>;
  readonly hasGit?: boolean;
  readonly cloneFailure?: Error;
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
  const sessions =
    options.sessions ?? createSessionStore<DoricSessionContext>();
  let dockerCreateCount = 0;

  return {
    docker,
    sandboxes,
    sandboxOptions,
    dockerCreateCount: () => dockerCreateCount,
    executor: createExecutor({
      sessions,
      createDockerClient: () => {
        dockerCreateCount += 1;

        return docker;
      },
      createSandbox: async (input) => {
        sandboxOptions.push(input);

        const sandbox = createFakeSandbox({
          hasGit: options.hasGit ?? true,
          cloneFailure: options.cloneFailure,
        });

        sandboxes.push(sandbox);

        return sandbox;
      },
    }),
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
    readonly message?: Message;
    readonly parts?: Message['parts'];
  } = {},
): RequestContext => {
  const contextId = options.contextId ?? randomUUID();
  const message =
    options.message ??
    createUserMessage({
      contextId,
      parts: options.parts ?? [createConfigPart()],
    });

  return new RequestContext(message, randomUUID(), contextId);
};

export const createUserMessage = (options: {
  readonly contextId?: string;
  readonly parts: Message['parts'];
}): Message => ({
  kind: 'message',
  messageId: randomUUID(),
  role: 'user',
  ...(options.contextId === undefined ? {} : { contextId: options.contextId }),
  parts: options.parts,
});

export const createConfigPart = (
  overrides: Partial<AgentConfig['github']> & {
    readonly repoUrl?: string;
    readonly token?: string;
  } = {},
): Message['parts'][number] => ({
  kind: 'data',
  data: {
    type: 'config',
    data: createConfig(overrides),
  },
});

export const createConfig = (
  options: {
    readonly repoUrl?: string;
    readonly token?: string;
  } = {},
): AgentConfig => ({
  github: {
    repo: {
      url: options.repoUrl ?? 'https://github.com/example/repo',
    },
    token: options.token ?? 'github-token',
  },
  providers: [
    {
      id: 'openai',
      type: 'openai',
      token: 'provider-token',
    },
  ],
  models: [
    {
      id: 'default',
      provider: 'openai',
      model: 'gpt-5',
    },
  ],
  tasks: [
    {
      id: 'coding',
      model: 'default',
    },
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
