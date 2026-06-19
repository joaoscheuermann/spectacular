import { Buffer } from 'node:buffer';
import { TextDecoder } from 'node:util';

import type {
  ContainerRef,
  CreateContainerInput,
  DockerClient,
  ExecInput,
} from 'docker';

import type {
  CreateSandboxOptions,
  GitAuth,
  SandboxDiffInput,
  SandboxExecInput,
  SandboxExecResult,
  SandboxNetworkPolicy,
  SandboxSession,
} from './types/sandbox.js';
import { extractFirstFile, packFile } from './utils/tar.js';

type SandboxState = {
  readonly docker: DockerClient;
  readonly container: ContainerRef;
  readonly root: string;
  repoPath: string | undefined;
  disposed: boolean;
};

const decoder = new TextDecoder();
const CONTAINER_NAME_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.-]+$/u;

/** Creates and starts an empty disposable coding sandbox container. */
export const createSandbox = async (
  options: CreateSandboxOptions,
): Promise<SandboxSession> => {
  validateName(options.name);

  const root = options.root ?? '/workspace';
  const container = await options.docker.createContainer(
    containerInput(options, root),
    { timeoutMs: options.timeoutMs },
  );

  try {
    await options.docker.startContainer(container, {
      timeoutMs: options.timeoutMs,
    });
  } catch (cause) {
    await options.docker
      .removeContainer(container, {
        force: true,
        volumes: true,
        timeoutMs: options.timeoutMs,
      })
      .catch(() => undefined);
    throw cause;
  }

  return session({
    docker: options.docker,
    container,
    root,
    repoPath: undefined,
    disposed: false,
  });
};

const session = (state: SandboxState): SandboxSession => ({
  id: state.container.id,
  root: state.root,

  exec(input) {
    ensureActive(state);

    return state.docker.exec(state.container, dockerExec(state, input));
  },

  async cloneRepo(input) {
    ensureActive(state);

    const directory = resolvePath(state.root, input.directory ?? 'repo');

    await checked(
      state,
      {
        cmd: gitCommand([], 'clone', [
          ...(input.branch === undefined ? [] : ['--branch', input.branch]),
          input.url,
          directory,
        ]),
        env: gitAuthEnv(input.auth),
        cwd: state.root,
        timeoutMs: input.timeoutMs,
      },
      'git clone failed',
    );

    if (input.commit !== undefined) {
      await checked(
        state,
        {
          cmd: gitCommand(['-C', directory], 'fetch', ['origin', input.commit]),
          env: gitAuthEnv(input.auth),
          timeoutMs: input.timeoutMs,
        },
        'git fetch failed',
      );
      await checked(
        state,
        {
          cmd: ['git', '-C', directory, 'checkout', '--detach', input.commit],
          timeoutMs: input.timeoutMs,
        },
        'git checkout failed',
      );
    }

    const resolved = await checked(
      state,
      {
        cmd: ['git', '-C', directory, 'rev-parse', 'HEAD'],
        timeoutMs: input.timeoutMs,
      },
      'git rev-parse failed',
    );

    state.repoPath = directory;

    return {
      path: directory,
      commit: resolved.stdout.trim(),
    };
  },

  async readFile(path) {
    return decoder.decode(await getFile(state, path));
  },

  writeFile(path, content) {
    return putFile(state, path, content);
  },

  putFile(path, content) {
    return putFile(state, path, content);
  },

  getFile(path) {
    return getFile(state, path);
  },

  async diff(input: SandboxDiffInput = {}) {
    ensureActive(state);

    const result = await state.docker.exec(
      state.container,
      dockerExec(state, {
        cmd: ['git', 'diff'],
        cwd: input.cwd ?? state.repoPath ?? state.root,
      }),
    );

    return result.stdout;
  },

  async dispose() {
    if (state.disposed) {
      return;
    }

    await state.docker.removeContainer(state.container, {
      force: true,
      volumes: true,
    });

    state.disposed = true;
  },
});

const putFile = async (
  state: SandboxState,
  path: string,
  content: string | Uint8Array,
): Promise<void> => {
  ensureActive(state);

  const absolute = resolvePath(state.root, path);
  const target = splitPath(absolute);
  const data =
    typeof content === 'string' ? Buffer.from(content, 'utf8') : content;

  await checked(
    state,
    { cmd: ['mkdir', '-p', target.directory], cwd: state.root },
    'mkdir failed',
  );
  await state.docker.putArchive(state.container, {
    path: target.directory,
    archive: packFile(target.name, data),
  });
};

const getFile = async (
  state: SandboxState,
  path: string,
): Promise<Uint8Array> => {
  ensureActive(state);

  const archive = await state.docker.getArchive(state.container, {
    path: resolvePath(state.root, path),
  });

  return extractFirstFile(archive).data;
};

const checked = async (
  state: SandboxState,
  input: SandboxExecInput,
  message: string,
): Promise<SandboxExecResult> => {
  const result = await state.docker.exec(
    state.container,
    dockerExec(state, input),
  );

  if (result.exitCode !== 0) {
    throw new Error(`${message}: ${result.stderr || result.stdout}`.trim());
  }

  return result;
};

const dockerExec = (
  state: SandboxState,
  input: SandboxExecInput,
): ExecInput => ({
  cmd: input.cmd,
  env: input.env,
  workingDir: input.cwd ?? state.repoPath ?? state.root,
  user: input.user,
  timeoutMs: input.timeoutMs,
  signal: input.signal,
  tty: input.tty,
});

const containerInput = (
  options: CreateSandboxOptions,
  root: string,
): CreateContainerInput => {
  const network = options.network ?? { mode: 'disabled' };

  return {
    name: options.name,
    image: options.image,
    cmd: ['sh', '-lc', 'while :; do sleep 3600; done'],
    workingDir: root,
    user: options.resources?.user,
    labels: {
      'doric.sandbox': 'true',
      'doric.sandbox.root': root,
    },
    hostConfig: compact({
      AutoRemove: false,
      Binds: [],
      NetworkMode: networkMode(network),
      Memory: options.resources?.memoryBytes,
      NanoCpus: options.resources?.nanoCpus,
      CpuPeriod: options.resources?.cpuPeriod,
      CpuQuota: options.resources?.cpuQuota,
      PidsLimit: options.resources?.pidsLimit,
    }),
    networkDisabled: network.mode === 'disabled',
  };
};

const validateName = (name: string | undefined): void => {
  if (name === undefined || CONTAINER_NAME_PATTERN.test(name)) {
    return;
  }

  throw new Error(
    `Docker container name must match ${CONTAINER_NAME_PATTERN}: ${name}`,
  );
};

const networkMode = (policy: SandboxNetworkPolicy): string => {
  if (policy.mode === 'disabled') {
    return 'none';
  }

  if (policy.mode === 'internal') {
    return policy.networkName;
  }

  return 'bridge';
};

const compact = (
  values: Readonly<Record<string, unknown>>,
): Record<string, unknown> =>
  Object.fromEntries(
    Object.entries(values).filter((entry) => entry[1] !== undefined),
  );

const gitCommand = (
  globalOptions: readonly string[],
  subcommand: string,
  suffix: readonly string[],
): readonly string[] => {
  return ['git', ...globalOptions, subcommand, ...suffix];
};

const gitAuthEnv = (
  auth: GitAuth | undefined,
): readonly string[] | undefined => {
  if (auth === undefined) {
    return undefined;
  }

  return [
    'GIT_CONFIG_COUNT=1',
    'GIT_CONFIG_KEY_0=http.extraHeader',
    `GIT_CONFIG_VALUE_0=${authorization(auth)}`,
  ];
};

const authorization = (auth: GitAuth): string => {
  const credential =
    auth.kind === 'token'
      ? `${auth.username ?? 'x-access-token'}:${auth.token}`
      : `${auth.username}:${auth.password}`;

  return `Authorization: Basic ${Buffer.from(credential).toString('base64')}`;
};

const resolvePath = (root: string, path: string): string => {
  const raw = path.startsWith('/') ? path : `${root}/${path}`;
  const parts = raw.split('/').filter((part) => part.length > 0);
  const rootParts = root.split('/').filter((part) => part.length > 0);

  if (parts.includes('..')) {
    throw new Error(`Sandbox paths must not contain '..': ${path}`);
  }

  if (
    rootParts.some((part, index) => parts[index] !== part) ||
    parts.length < rootParts.length
  ) {
    throw new Error(`Sandbox paths must stay under ${root}: ${path}`);
  }

  return `/${parts.join('/')}`;
};

const splitPath = (
  path: string,
): {
  readonly directory: string;
  readonly name: string;
} => {
  const index = path.lastIndexOf('/');

  if (index <= 0 || index === path.length - 1) {
    throw new Error(`Sandbox file path must include a file name: ${path}`);
  }

  return {
    directory: path.slice(0, index),
    name: path.slice(index + 1),
  };
};

const ensureActive = (state: SandboxState): void => {
  if (state.disposed) {
    throw new Error(
      `Sandbox container has been disposed: ${state.container.id}`,
    );
  }
};
