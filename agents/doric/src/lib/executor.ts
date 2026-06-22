import {
  A2AError,
  type AgentExecutor,
  type ExecutionEventBus,
  type RequestContext,
} from '@a2a-js/sdk/server';

import type { AgentConfig } from 'config';
import {
  ConfigParseError,
  parseInitialMessageConfig,
  parseMessageConfigUpdate,
} from 'config';

import type { SessionStore } from 'session';

import { createDockerClient } from 'docker';

import { createSandbox } from 'sandbox';
import type { ClonedRepo, SandboxSession } from 'sandbox';

import {
  DEFAULT_SANDBOX_IMAGE,
  GIT_INSTALL_COMMAND,
  GIT_PROBE_COMMAND,
} from './constants/sandbox.js';

import {
  checkingGitMessage,
  checkingSessionMessage,
  cloningRepositoryMessage,
  creatingSandboxMessage,
  installingGitMessage,
  sessionReadyMessage,
  taskCreatedMessage,
  updatingSessionConfigMessage,
  usingExistingSessionMessage,
} from './messages/index.js';

export type DoricSessionContext = {
  readonly repo: ClonedRepo;
  config: AgentConfig;
  readonly sandbox: SandboxSession;
};

export type DoricExecutorDependencies = {
  readonly sessions: SessionStore<DoricSessionContext>;
  readonly createDockerClient: typeof createDockerClient;
  readonly createSandbox: typeof createSandbox;
};

type ResolvedDependencies = {
  readonly sessions: SessionStore<DoricSessionContext>;
  readonly createDockerClient: typeof createDockerClient;
  readonly createSandbox: typeof createSandbox;
};

type Progress = {
  readonly eventBus: ExecutionEventBus;
  readonly taskId: string;
  readonly contextId: string;
};

/** Creates Doric's A2A executor with its required session store dependency. */
export const createExecutor = ({
  sessions,
  createSandbox,
  createDockerClient,
}: DoricExecutorDependencies): AgentExecutor => {
  return {
    async execute(requestContext, eventBus) {
      eventBus.publish(
        taskCreatedMessage(requestContext.taskId, requestContext.contextId),
      );

      await ensureSession(
        requestContext,
        {
          sessions,
          createSandbox,
          createDockerClient,
        },
        {
          eventBus,
          taskId: requestContext.taskId,
          contextId: requestContext.contextId,
        },
      );

      return eventBus.finished();
    },
    async cancelTask() {},
  };
};

// Makes sure the session is initialized or stored!
const ensureSession = async (
  requestContext: RequestContext,
  dependencies: ResolvedDependencies,
  progress: Progress,
): Promise<DoricSessionContext> => {
  progress.eventBus.publish(
    checkingSessionMessage(progress.taskId, progress.contextId),
  );

  const existing = dependencies.sessions.load(requestContext.contextId);

  if (existing !== undefined) {
    const session = await existing;
    const config = parseConfigUpdate(requestContext);

    if (config !== undefined) {
      progress.eventBus.publish(
        updatingSessionConfigMessage(progress.taskId, progress.contextId),
      );
      session.config = config;
    }

    progress.eventBus.publish(
      usingExistingSessionMessage(progress.taskId, progress.contextId),
    );

    return session;
  }

  const config = parseInitialConfig(requestContext);

  return dependencies.sessions.getOrCreate(
    requestContext.contextId,
    async () => {
      const docker = dependencies.createDockerClient();

      progress.eventBus.publish(
        creatingSandboxMessage(progress.taskId, progress.contextId),
      );

      const sandbox = await dependencies.createSandbox({
        docker,
        image: DEFAULT_SANDBOX_IMAGE,
        name: `doric-${requestContext.contextId.replaceAll(/[^A-Za-z0-9_.-]/gu, '-')}`,
        network: { mode: 'bridge' },
      });

      await ensureGit(sandbox, progress);

      progress.eventBus.publish(
        cloningRepositoryMessage(progress.taskId, progress.contextId),
      );

      const repo = await sandbox.cloneRepo({
        url: config.github.repo.url,
        auth: { kind: 'token', token: config.github.token },
      });

      progress.eventBus.publish(
        sessionReadyMessage(progress.taskId, progress.contextId),
      );

      return { config, sandbox, repo };
    },
  );
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

const parseConfigUpdate = (
  requestContext: RequestContext,
): AgentConfig | undefined => {
  try {
    return parseMessageConfigUpdate(requestContext.userMessage);
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

const ensureGit = async (
  sandbox: SandboxSession,
  progress: Progress,
): Promise<void> => {
  const formatExecFailure = (
    message: string,
    result: Awaited<ReturnType<SandboxSession['exec']>>,
  ): string => `${message}: ${result.stderr || result.stdout}`.trim();

  progress.eventBus.publish(
    checkingGitMessage(progress.taskId, progress.contextId),
  );

  const probe = await sandbox.exec({ cmd: GIT_PROBE_COMMAND });

  if (probe.exitCode === 0) {
    return;
  }

  progress.eventBus.publish(
    installingGitMessage(progress.taskId, progress.contextId),
  );

  const install = await sandbox.exec({ cmd: GIT_INSTALL_COMMAND });

  if (install.exitCode !== 0) {
    throw new Error(formatExecFailure('git install failed', install));
  }

  const verified = await sandbox.exec({ cmd: GIT_PROBE_COMMAND });

  if (verified.exitCode !== 0) {
    throw new Error(formatExecFailure('git probe failed', verified));
  }
};
