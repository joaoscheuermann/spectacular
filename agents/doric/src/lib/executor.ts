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

import {
  createFetchTransport as createLlmFetchTransport,
  createCodexProvider,
  createOpenAiProvider,
  createOpenRouterProvider,
} from 'llms';
import type { LlmProvider } from 'llms';

import { codexCredentialFromToken } from 'oauth';

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
  taskCreatedMessage,
  updatingSessionConfigMessage,
  usingExistingSessionMessage,
} from './messages/index.js';

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
  readonly promptRunner?: PromptRunner;
  readonly createProvider?: ProviderFactory;
};

type ResolvedDependencies = {
  readonly sessions: SessionStore<DoricSessionContext>;
  readonly createDockerClient: typeof createDockerClient;
  readonly createSandbox: typeof createSandbox;
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
  promptRunner = runPromptWorkflow,
  createProvider = createProviderFromConfig,
}: DoricExecutorDependencies): AgentExecutor => {
  const dependencies = {
    sessions,
    createSandbox,
    createDockerClient,
    promptRunner,
    createProvider,
  };

  return {
    async execute(requestContext, eventBus) {
      eventBus.publish(
        taskCreatedMessage(requestContext.taskId, requestContext.contextId),
      );

      const progress = {
        eventBus,
        taskId: requestContext.taskId,
        contextId: requestContext.contextId,
      };
      const userPrompt = userPromptText(requestContext);

      const session = await ensureSession(
        requestContext,
        dependencies,
        progress,
      );

      await runPrompt(userPrompt, session, dependencies, progress);

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

const runPrompt = async (
  userPrompt: string,
  session: DoricSessionContext,
  dependencies: ResolvedDependencies,
  progress: Progress,
): Promise<void> => {
  const { model, provider } = resolvePromptModel(session.config);
  const promptProvider = await dependencies.createProvider(provider);

  progress.eventBus.publish(
    runningPromptWorkflowMessage(progress.taskId, progress.contextId),
  );

  const artifact = await dependencies.promptRunner(
    createPromptArtifact(userPrompt),
    {
      provider: promptProvider,
      model: model.model,
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
};

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

const createProviderFromConfig = async (
  provider: ProviderConfig,
): Promise<LlmProvider> => {
  const transport = createLlmFetchTransport();

  if (provider.type === 'openai') {
    const token = requiredToken(provider);

    return token.toLowerCase().startsWith('bearer ')
      ? createOpenAiProvider({ transport, authorization: token })
      : createOpenAiProvider({ transport, apiKey: token });
  }

  if (provider.type === 'openrouter') {
    return createOpenRouterProvider({
      transport,
      apiKey: bearerValue(requiredToken(provider)),
    });
  }

  if (provider.type === 'codex') {
    const credential = codexCredentialFromToken(requiredToken(provider));

    return createCodexProvider({
      transport,
      authorization: credential.authorization,
      chatGptAccountId: credential.accountId,
      fedramp: credential.fedramp,
    });
  }

  throw A2AError.invalidParams(
    `Unsupported provider type "${provider.type}".`,
    {
      code: 'unsupported_provider_type',
      path: 'message.metadata.configuration.providers.type',
    },
  );
};

const requiredToken = (provider: ProviderConfig): string => {
  const token = provider.token?.trim();

  if (token === undefined || token === '') {
    throw A2AError.invalidParams(
      `Provider "${provider.id}" requires a token.`,
      {
        code: 'missing_provider_token',
        path: 'message.metadata.configuration.providers.token',
      },
    );
  }

  return token;
};

const bearerValue = (token: string): string => {
  const trimmed = token.trim();

  return trimmed.toLowerCase().startsWith('bearer ')
    ? trimmed.slice('bearer '.length).trimStart()
    : trimmed;
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
