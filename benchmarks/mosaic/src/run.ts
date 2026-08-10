import {
  createFetchTransport,
  createUnifiedProvider,
  type HttpTransport,
  type LlmProvider,
} from 'llms';
import { readFileSync, unlinkSync } from 'node:fs';
import pino, { type Logger } from 'pino';

export type RunMode = 'direct' | 'mosaic';

export interface RunRequest {
  readonly prompt: string;
  readonly cwd: string;
  readonly signal?: AbortSignal;
}

export type RunEvent =
  | { readonly type: 'message_delta'; readonly delta: string }
  | {
      readonly type: 'tool_started';
      readonly callId: string;
      readonly name: string;
      readonly input?: unknown;
    }
  | {
      readonly type: 'tool_completed';
      readonly callId: string;
      readonly name: string;
      readonly output?: unknown;
    }
  | {
      readonly type: 'tool_failed';
      readonly callId: string;
      readonly name: string;
    }
  | { readonly type: 'status'; readonly status: string };

export type Emit = (event: RunEvent) => void | Promise<void>;

export interface Runner {
  run(request: RunRequest, emit: Emit): Promise<void>;
}

export interface ProviderProfile {
  readonly provider: LlmProvider;
  readonly model: string;
}

export interface ProviderOptions {
  readonly environment?: NodeJS.ProcessEnv;
  readonly transport?: HttpTransport;
  readonly logger?: Logger;
}

export const defaultModel = 'openai/gpt-5.6-luna';

const sensitiveName =
  /(?:master|private|api|access)[_-]?key|auth(?:orization)?|bearer|token|secret|password|credentials?|cookie/iu;

const scrub = (environment: NodeJS.ProcessEnv): void => {
  for (const name of Object.keys(environment)) {
    if (sensitiveName.test(name)) delete environment[name];
  }
};

const credentialFromFile = (environment: NodeJS.ProcessEnv): string => {
  const path = environment.OPENROUTER_API_KEY_FILE;

  if (path === undefined || path.trim() === '') {
    throw new Error(
      'OPENROUTER_API_KEY_FILE is required when using process.env.',
    );
  }

  try {
    return readFileSync(path, 'utf8');
  } finally {
    unlinkSync(path);
  }
};

const credential = (
  environment: NodeJS.ProcessEnv,
  usesProcessEnvironment: boolean,
): string => {
  if (!usesProcessEnvironment) return environment.OPENROUTER_API_KEY ?? '';

  try {
    return credentialFromFile(environment);
  } finally {
    scrub(environment);
  }
};

/** Composes the benchmark's credential-safe Unified OpenRouter profile. */
export const createProvider = (
  options: ProviderOptions = {},
): ProviderProfile => {
  const environment = options.environment ?? process.env;
  const apiKey = credential(
    environment,
    options.environment === undefined || options.environment === process.env,
  );
  const model = environment.OPENROUTER_MODEL?.trim() || defaultModel;
  const baseUrl = environment.OPENROUTER_BASE_URL?.trim() || undefined;
  const logger = options.logger ?? pino({ enabled: false });

  return {
    model,
    provider: createUnifiedProvider({
      transport: options.transport ?? createFetchTransport(),
      ...(baseUrl === undefined ? {} : { baseUrl }),
      apiKey: () => apiKey,
      logger,
      upstreamModel: defaultModel,
    }),
  };
};
