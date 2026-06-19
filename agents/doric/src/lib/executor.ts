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

import {
  DEFAULT_IMAGE,
  GIT_INSTALL_COMMAND,
  GIT_PROBE_COMMAND,
} from './constants/sandbox.js';
import { createHelloWorldMessage } from './messages/hello-world.js';

export type DoricSessionContext = {
  readonly config: AgentConfig;
  readonly sandbox: SandboxSession;
  readonly repo: ClonedRepo;
};

export type DoricExecutorDependencies = {
  readonly sessions: SessionStore<DoricSessionContext>;
  readonly createDockerClient?: typeof createDockerClient;
  readonly createSandbox?: (
    options: CreateSandboxOptions,
  ) => Promise<SandboxSession>;
};

/** Creates Doric's A2A executor with its required session store dependency. */
export const createExecutor = (
  dependencies: DoricExecutorDependencies,
): AgentExecutor => {
  const sessions = dependencies.sessions;
  const dockerClient = dependencies.createDockerClient ?? createDockerClient;
  const sandbox = dependencies.createSandbox ?? createSandbox;

  return {
    async execute(requestContext, eventBus) {
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

type ResolvedDependencies = {
  readonly sessions: SessionStore<DoricSessionContext>;
  readonly createDockerClient: typeof createDockerClient;
  readonly createSandbox: (
    options: CreateSandboxOptions,
  ) => Promise<SandboxSession>;
};

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
