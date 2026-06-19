import { randomUUID } from 'node:crypto';

import type { Message } from '@a2a-js/sdk';
import {
  A2AError,
  type AgentExecutor,
  type RequestContext,
} from '@a2a-js/sdk/server';
import { ConfigParseError, parseInitialMessageConfig } from 'config';
import type { AgentConfig } from 'config';
import { createDockerClient } from 'docker';
import { createSandbox } from 'sandbox';
import type { ClonedRepo, CreateSandboxOptions, SandboxSession } from 'sandbox';
import type { SessionStore } from 'session';

import { HELLO_WORLD_TEXT } from './card.js';

export type DoricSessionContext = {
  readonly config: AgentConfig;
  readonly sandbox: SandboxSession;
  readonly repo: ClonedRepo;
};

export type DoricExecutorDependencies = {
  readonly sessions?: SessionStore<DoricSessionContext>;
  readonly createDockerClient?: typeof createDockerClient;
  readonly createSandbox?: (
    options: CreateSandboxOptions,
  ) => Promise<SandboxSession>;
};

const DEFAULT_IMAGE = 'node:slim';
const GIT_PROBE_COMMAND = ['sh', '-lc', 'command -v git >/dev/null 2>&1'];
const GIT_INSTALL_COMMAND = [
  'sh',
  '-lc',
  'apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*',
];
const MISSING_SESSION_STORE_MESSAGE =
  'Doric executor requires a session store. Pass sessions to createExecutor({ sessions }).';

/** Creates Doric's A2A executor. Execution requires an injected session store. */
export const createExecutor = (
  dependencies: DoricExecutorDependencies = {},
): AgentExecutor => {
  const sessions = dependencies.sessions;
  const dockerClient = dependencies.createDockerClient ?? createDockerClient;
  const sandbox = dependencies.createSandbox ?? createSandbox;

  return {
    async execute(requestContext, eventBus) {
      if (sessions === undefined) {
        throw new Error(MISSING_SESSION_STORE_MESSAGE);
      }

      await ensureSession(requestContext, {
        sessions,
        createDockerClient: dockerClient,
        createSandbox: sandbox,
      });

      eventBus.publish(createHelloWorldMessage(requestContext.contextId));
      eventBus.finished();
    },
    cancelTask: () => Promise.resolve(),
  };
};

type ResolvedDependencies = Required<DoricExecutorDependencies>;

const ensureSession = (
  requestContext: RequestContext,
  dependencies: ResolvedDependencies,
): Promise<DoricSessionContext> => {
  const existing = dependencies.sessions.load(requestContext.contextId);

  if (existing !== undefined) {
    return existing;
  }

  const config = parseInitialConfig(requestContext);

  return dependencies.sessions.getOrCreate(requestContext.contextId, () =>
    createContext(config, dependencies),
  );
};

const createContext = async (
  config: AgentConfig,
  dependencies: ResolvedDependencies,
): Promise<DoricSessionContext> => {
  const docker = dependencies.createDockerClient();
  let sandbox: SandboxSession | undefined;

  try {
    sandbox = await dependencies.createSandbox({
      docker,
      image: DEFAULT_IMAGE,
      network: { mode: 'bridge' },
    });
    await ensureGit(sandbox);

    const repo = await sandbox.cloneRepo({
      url: config.github.repo.url,
      auth: { kind: 'token', token: config.github.token },
    });

    return { config, sandbox, repo };
  } catch (error) {
    await sandbox?.dispose().catch(() => undefined);
    throw error;
  }
};

const parseInitialConfig = (requestContext: RequestContext): AgentConfig => {
  try {
    return parseInitialMessageConfig(requestContext.userMessage);
  } catch (error) {
    if (error instanceof ConfigParseError) {
      throw A2AError.invalidParams(error.message, {
        code: error.code,
        path: error.path,
      });
    }

    throw error;
  }
};

const ensureGit = async (sandbox: SandboxSession): Promise<void> => {
  const probe = await sandbox.exec({ cmd: GIT_PROBE_COMMAND });

  if (probe.exitCode === 0) {
    return;
  }

  const install = await sandbox.exec({ cmd: GIT_INSTALL_COMMAND });

  if (install.exitCode !== 0) {
    throw new Error(formatExecFailure('git install failed', install));
  }

  const verified = await sandbox.exec({ cmd: GIT_PROBE_COMMAND });

  if (verified.exitCode !== 0) {
    throw new Error(formatExecFailure('git probe failed', verified));
  }
};

const formatExecFailure = (
  message: string,
  result: Awaited<ReturnType<SandboxSession['exec']>>,
): string => `${message}: ${result.stderr || result.stdout}`.trim();

const createHelloWorldMessage = (contextId: string): Message => ({
  kind: 'message',
  messageId: randomUUID(),
  role: 'agent',
  parts: [{ kind: 'text', text: HELLO_WORLD_TEXT }],
  contextId,
});
