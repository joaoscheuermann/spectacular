import type { DockerClient } from 'docker';

export type SandboxNetworkPolicy =
  | { readonly mode: 'disabled' }
  | { readonly mode: 'internal'; readonly networkName: string }
  | { readonly mode: 'bridge' };

export type SandboxResources = {
  readonly memoryBytes?: number;
  readonly nanoCpus?: number;
  readonly cpuPeriod?: number;
  readonly cpuQuota?: number;
  readonly pidsLimit?: number;
  readonly user?: string;
};

export type SandboxExecInput = {
  readonly cmd: readonly string[];
  readonly cwd?: string;
  readonly env?: readonly string[];
  readonly user?: string;
  readonly timeoutMs?: number;
  readonly signal?: AbortSignal;
  readonly tty?: boolean;
};

export type SandboxExecResult = {
  readonly exitCode: number | null;
  readonly stdout: string;
  readonly stderr: string;
  readonly stdoutBytes: Uint8Array;
  readonly stderrBytes: Uint8Array;
};

export type GitAuth =
  | {
      readonly kind: 'token';
      readonly token: string;
      readonly username?: string;
    }
  | {
      readonly kind: 'basic';
      readonly username: string;
      readonly password: string;
    };

export type CloneRepoInput = {
  readonly url: string;
  readonly directory?: string;
  readonly branch?: string;
  readonly commit?: string;
  readonly auth?: GitAuth;
  readonly timeoutMs?: number;
};

export type ClonedRepo = {
  readonly path: string;
  readonly commit: string;
};

export type CreateSandboxOptions = {
  readonly docker: DockerClient;
  readonly image: string;
  readonly name?: string;
  readonly root?: string;
  readonly resources?: SandboxResources;
  readonly network?: SandboxNetworkPolicy;
  readonly timeoutMs?: number;
};

export type SandboxDiffInput = {
  readonly cwd?: string;
};

export interface SandboxSession {
  readonly id: string;
  readonly root: string;
  exec(input: SandboxExecInput): Promise<SandboxExecResult>;
  cloneRepo(input: CloneRepoInput): Promise<ClonedRepo>;
  readFile(path: string): Promise<string>;
  writeFile(path: string, content: string): Promise<void>;
  putFile(path: string, bytes: Uint8Array): Promise<void>;
  getFile(path: string): Promise<Uint8Array>;
  diff(input?: SandboxDiffInput): Promise<string>;
  dispose(): Promise<void>;
}
