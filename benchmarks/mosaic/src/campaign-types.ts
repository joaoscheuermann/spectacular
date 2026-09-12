export type Benchmark = 'skillsbench' | 'terminalbench';

export type CampaignAction = 'check' | 'smoke' | 'pilot' | 'run';

export type Arm = 'mosaic-direct' | 'mosaic';

export const armOrder = [
  'mosaic',
  'mosaic-direct',
] as const satisfies readonly Arm[];

export type Command = {
  readonly file: string;
  readonly args: readonly string[];
  readonly cwd: string;
  readonly env?: Readonly<Record<string, string>>;
};

export type CommandResult = {
  readonly code: number;
  readonly stdout: string;
  readonly stderr: string;
};

export type CommandRunner = (command: Command) => Promise<CommandResult>;

export type Check = {
  readonly name: string;
  readonly ok: boolean;
  readonly detail: string;
};

export type CampaignCheck = {
  readonly ok: boolean;
  readonly checks: readonly Check[];
};

export type CampaignMetadata = {
  readonly action: 'smoke' | 'pilot' | 'run';
  readonly benchmark: Benchmark;
  readonly benchflowVersion: '0.6.5';
  readonly campaignId: string;
  readonly source: {
    readonly repo: string;
    readonly path: string;
    readonly ref: string;
  };
  readonly expectedTasks: number;
  readonly agent: Arm;
  readonly model: string;
  readonly effort: 'low';
  readonly sandbox: 'docker';
  readonly concurrency: 1;
  readonly buildConcurrency: 1;
  readonly retries: 0;
  readonly loopStrategy: 'single-shot';
  readonly usageTracking: 'required';
  readonly skillMode: 'with-skill' | 'no-skill';
  readonly digests: Readonly<Record<string, string>>;
};

export type ArmRun = {
  readonly arm: Arm;
  readonly directory: string;
  readonly command: Command;
  readonly result: CommandResult;
};

export type CampaignRun = {
  readonly directory: string;
  readonly arms: readonly ArmRun[];
};

export type CampaignOptions = {
  readonly benchmark: Benchmark;
  readonly action: CampaignAction;
  readonly rootDir: string;
  readonly yesPaidRun?: boolean;
  readonly skillsbenchReport?: string;
  readonly runner?: CommandRunner;
  readonly environment?: NodeJS.ProcessEnv;
};
