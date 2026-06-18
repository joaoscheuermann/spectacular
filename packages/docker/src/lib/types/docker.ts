export type DockerConnection =
  | {
      readonly kind: 'unix';
      readonly socketPath?: string;
    }
  | {
      readonly kind: 'namedPipe';
      readonly pipePath?: string;
    };

export type DockerRequestQuery = Readonly<
  Record<string, string | number | boolean | undefined>
>;

export type DockerRequestOptions = {
  readonly signal?: AbortSignal;
  readonly timeoutMs?: number;
};

export type DockerTransportRequest = DockerRequestOptions & {
  readonly method: string;
  readonly path: string;
  readonly query?: DockerRequestQuery;
  readonly headers?: Readonly<Record<string, string>>;
  readonly body?: unknown;
};

export type DockerResponse = {
  readonly status: number;
  readonly headers: Readonly<
    Record<string, string | readonly string[] | undefined>
  >;
  readonly body: Uint8Array;
};

export type DockerTransport = (
  request: DockerTransportRequest,
) => Promise<DockerResponse>;

export type DockerVersion = {
  readonly version?: string;
  readonly apiVersion?: string;
  readonly minApiVersion?: string;
  readonly gitCommit?: string;
  readonly goVersion?: string;
  readonly os?: string;
  readonly arch?: string;
  readonly kernelVersion?: string;
  readonly experimental?: boolean;
  readonly raw: Readonly<Record<string, unknown>>;
};

export type CreateContainerInput = {
  readonly image: string;
  readonly cmd?: readonly string[];
  readonly env?: readonly string[];
  readonly workingDir?: string;
  readonly labels?: Readonly<Record<string, string>>;
  readonly user?: string;
  readonly name?: string;
  readonly hostConfig?: Readonly<Record<string, unknown>>;
  readonly networkDisabled?: boolean;
};

export type ContainerRef = {
  readonly id: string;
  readonly warnings: readonly string[];
};

export type RemoveContainerOptions = DockerRequestOptions & {
  readonly force?: boolean;
  readonly volumes?: boolean;
  readonly link?: boolean;
};

export type ExecInput = DockerRequestOptions & {
  readonly cmd: readonly string[];
  readonly env?: readonly string[];
  readonly workingDir?: string;
  readonly user?: string;
  readonly tty?: boolean;
};

export type ExecResult = {
  readonly exitCode: number | null;
  readonly stdout: string;
  readonly stderr: string;
  readonly stdoutBytes: Uint8Array;
  readonly stderrBytes: Uint8Array;
};

export type ArchiveWriteInput = {
  readonly path: string;
  readonly archive: Uint8Array;
  readonly noOverwriteDirNonDir?: boolean;
  readonly copyUidGid?: boolean;
};

export type ArchiveReadInput = {
  readonly path: string;
};

export interface DockerClient {
  ping(options?: DockerRequestOptions): Promise<void>;
  version(options?: DockerRequestOptions): Promise<DockerVersion>;
  createContainer(
    input: CreateContainerInput,
    options?: DockerRequestOptions,
  ): Promise<ContainerRef>;
  startContainer(
    container: ContainerRef | string,
    options?: DockerRequestOptions,
  ): Promise<void>;
  removeContainer(
    container: ContainerRef | string,
    options?: RemoveContainerOptions,
  ): Promise<void>;
  exec(container: ContainerRef | string, input: ExecInput): Promise<ExecResult>;
  putArchive(
    container: ContainerRef | string,
    input: ArchiveWriteInput,
    options?: DockerRequestOptions,
  ): Promise<void>;
  getArchive(
    container: ContainerRef | string,
    input: ArchiveReadInput,
    options?: DockerRequestOptions,
  ): Promise<Uint8Array>;
}
