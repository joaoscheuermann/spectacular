import {
  A2AError,
  type AgentExecutor,
  type ExecutionEventBus,
  type RequestContext,
} from '@a2a-js/sdk/server';

import { renderArtifact } from 'artifacts';

import type { AgentConfig, ModelConfig, ProviderConfig } from 'config';
import {
  ConfigParseError,
  parseInitialMessageConfig,
  parseMessageConfigUpdate,
} from 'config';

import type { LlmProvider } from 'llms';

import type { SessionStore } from 'session';

import { createDockerClient } from 'docker';

import { createSandbox } from 'sandbox';
import type { ClonedRepo, SandboxSession } from 'sandbox';

import {
  createPromptArtifact,
  prompt as runPromptWorkflow,
} from 'workflow-prompt';
import type { PromptArtifact, PromptWorkflowOptions } from 'workflow-prompt';

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
  promptArtifactMessage,
  promptCompletedMessage,
  promptInputRequiredMessage,
  runningPromptWorkflowMessage,
  sessionReadyMessage,
  taskCanceledMessage,
  taskCreatedMessage,
  updatingSessionConfigMessage,
  usingExistingSessionMessage,
} from './messages/index.js';
import { createProviderFromConfig } from './utils/provider.js';

export type DoricSessionContext = {
  readonly repo: ClonedRepo;
  config: AgentConfig;
  readonly sandbox: SandboxSession;
  lastPromptArtifact?: PromptArtifact;
};

export type PromptRunner = (
  artifact: PromptArtifact,
  options: PromptWorkflowOptions,
) => Promise<PromptArtifact>;

export type ProviderFactory = (
  provider: ProviderConfig,
) => LlmProvider | Promise<LlmProvider>;

export type DoricExecutorDependencies = {
  readonly sessions: SessionStore<DoricSessionContext>;
  readonly createDockerClient: typeof createDockerClient;
  readonly createSandbox: typeof createSandbox;
  readonly logger?: DoricExecutorLogger;
  readonly promptRunner?: PromptRunner;
  readonly createProvider?: ProviderFactory;
};

export type DoricExecutorLogger = {
  readonly info: (
    bindings: Record<string, unknown>,
    message: string,
  ) => void;
  readonly error: (
    bindings: Record<string, unknown>,
    message: string,
  ) => void;
};

type ResolvedDependencies = {
  readonly sessions: SessionStore<DoricSessionContext>;
  readonly createDockerClient: typeof createDockerClient;
  readonly createSandbox: typeof createSandbox;
  readonly logger: DoricExecutorLogger;
  readonly promptRunner: PromptRunner;
  readonly createProvider: ProviderFactory;
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
  logger = noopLogger,
  promptRunner = runPromptWorkflow,
  createProvider = createProviderFromConfig,
}: DoricExecutorDependencies): AgentExecutor => {
  const dependencies = {
    sessions,
    createSandbox,
    createDockerClient,
    logger,
    promptRunner,
    createProvider,
  };
  const activeContexts = new Map<string, string>();

  return {
    async execute(requestContext, eventBus) {
      const progress = {
        eventBus,
        taskId: requestContext.taskId,
        contextId: requestContext.contextId,
      };
      activeContexts.set(progress.taskId, progress.contextId);

      dependencies.logger.info(
        { ...logContext(progress), lifecycle: 'task.started' },
        'Doric task started',
      );

      try {
        eventBus.publish(
          taskCreatedMessage(requestContext.taskId, requestContext.contextId),
        );

        const userPrompt = userPromptText(requestContext);

        const session = await ensureSession(
          requestContext,
          dependencies,
          progress,
        );

        await runPrompt(userPrompt, session, dependencies, progress);

        dependencies.logger.info(
          { ...logContext(progress), lifecycle: 'task.finished' },
          'Doric task finished',
        );
      } catch (error) {
        dependencies.logger.error(
          {
            ...logContext(progress),
            lifecycle: 'task.failed',
            errorName: errorName(error),
          },
          'Doric task failed',
        );

        throw error;
      } finally {
        activeContexts.delete(progress.taskId);
      }

      return eventBus.finished();
    },
    async cancelTask(taskId, eventBus) {
      const contextId = activeContexts.get(taskId);

      if (contextId === undefined) {
        return;
      }

      eventBus.publish(taskCanceledMessage(taskId, contextId));
      eventBus.finished();
    },
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
  dependencies.logger.info(
    { ...logContext(progress), lifecycle: 'session.checking' },
    'Checking Doric session',
  );

  const existing = dependencies.sessions.load(requestContext.contextId);

  if (existing !== undefined) {
    const session = await existing;
    const config = parseConfigUpdate(requestContext);

    if (config !== undefined) {
      progress.eventBus.publish(
        updatingSessionConfigMessage(progress.taskId, progress.contextId),
      );
      dependencies.logger.info(
        { ...logContext(progress), lifecycle: 'session.config_updated' },
        'Updated Doric session config',
      );
      session.config = config;
    }

    progress.eventBus.publish(
      usingExistingSessionMessage(progress.taskId, progress.contextId),
    );
    dependencies.logger.info(
      { ...logContext(progress), lifecycle: 'session.reused' },
      'Reusing Doric session',
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
      dependencies.logger.info(
        { ...logContext(progress), lifecycle: 'sandbox.creating' },
        'Creating Doric sandbox',
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
      dependencies.logger.info(
        { ...logContext(progress), lifecycle: 'repo.cloning' },
        'Cloning Doric repository',
      );

      const repo = await sandbox.cloneRepo({
        url: config.github.repo.url,
        ...(config.github.repo.branch === undefined
          ? {}
          : { branch: config.github.repo.branch }),
        auth: { kind: 'token', token: config.github.token },
      });

      progress.eventBus.publish(
        sessionReadyMessage(progress.taskId, progress.contextId),
      );
      dependencies.logger.info(
        { ...logContext(progress), lifecycle: 'session.ready' },
        'Doric session ready',
      );

      return { config, sandbox, repo };
    },
  );
};

const runPrompt = async (
  userPrompt: string,
  session: DoricSessionContext,
  dependencies: ResolvedDependencies,
  progress: Progress,
): Promise<void> => {
  const { model, provider } = resolvePromptModel(session.config);
  const promptProvider = await dependencies.createProvider(provider);
  const effort = model.effort ?? model.reasoning;

  progress.eventBus.publish(
    runningPromptWorkflowMessage(progress.taskId, progress.contextId),
  );
  dependencies.logger.info(
    {
      ...logContext(progress),
      lifecycle: 'prompt.started',
      providerType: provider.type,
    },
    'Doric prompt workflow started',
  );

  const artifact = await dependencies.promptRunner(
    createPromptArtifact(userPrompt),
    {
      provider: promptProvider,
      model: model.model,
      ...(effort === undefined ? {} : { effort }),
      workspaceRoot: session.repo.path,
      sandbox: session.sandbox,
    },
  );
  const questions = artifact.data.openQuestions ?? [];
  const rendered = renderArtifact(artifact);

  session.lastPromptArtifact = artifact;

  progress.eventBus.publish(
    promptArtifactMessage(
      progress.taskId,
      progress.contextId,
      artifact,
      rendered,
    ),
  );

  progress.eventBus.publish(
    questions.length > 0
      ? promptInputRequiredMessage(
          progress.taskId,
          progress.contextId,
          questions,
        )
      : promptCompletedMessage(progress.taskId, progress.contextId),
  );
  dependencies.logger.info(
    {
      ...logContext(progress),
      lifecycle: 'prompt.finished',
      questionCount: questions.length,
      status: questions.length > 0 ? 'input-required' : 'completed',
    },
    'Doric prompt workflow finished',
  );
};

const noopLogger: DoricExecutorLogger = {
  info() {},
  error() {},
};

const logContext = (
  progress: Progress,
): { readonly taskId: string; readonly contextId: string } => ({
  taskId: progress.taskId,
  contextId: progress.contextId,
});

const errorName = (error: unknown): string =>
  error instanceof Error ? error.name : 'UnknownError';

const userPromptText = (requestContext: RequestContext): string => {
  const text = requestContext.userMessage.parts
    .filter((part) => part.kind === 'text')
    .map((part) => part.text)
    .join('\n')
    .trim();

  if (text === '') {
    throw A2AError.invalidParams('User message must include prompt text.', {
      code: 'missing_prompt_text',
      path: 'message.parts',
    });
  }

  return text;
};

const resolvePromptModel = (
  config: AgentConfig,
): { readonly model: ModelConfig; readonly provider: ProviderConfig } => {
  const task =
    config.tasks.find((candidate) => candidate.id === 'coding') ??
    config.tasks[0];

  if (task === undefined) {
    throw A2AError.invalidParams('Configuration must include a task.', {
      code: 'missing_task_config',
      path: 'message.metadata.configuration.tasks',
    });
  }

  const model = config.models.find((candidate) => candidate.id === task.model);

  if (model === undefined) {
    throw A2AError.invalidParams(
      `Configuration task "${task.id}" references unknown model "${task.model}".`,
      {
        code: 'unknown_model_config',
        path: 'message.metadata.configuration.tasks.model',
      },
    );
  }

  const provider = config.providers.find(
    (candidate) => candidate.id === model.provider,
  );

  if (provider === undefined) {
    throw A2AError.invalidParams(
      `Configuration model "${model.id}" references unknown provider "${model.provider}".`,
      {
        code: 'unknown_provider_config',
        path: 'message.metadata.configuration.models.provider',
      },
    );
  }

  return { model, provider };
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
