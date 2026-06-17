import { mkdir, appendFile } from 'node:fs/promises';
import { dirname } from 'node:path';

import { redactDiagnosticValue } from './utils/diagnostics.js';

export type LlmDebugRecord = {
  readonly target?: string;
  readonly provider?: string;
  readonly event: string;
  readonly fields?: Readonly<Record<string, unknown>>;
};

export type LlmDebugLogger = {
  log(record: LlmDebugRecord): Promise<void>;
};

export type LlmDebugLoggerDeps = {
  readonly appendFile?: typeof appendFile;
  readonly mkdir?: typeof mkdir;
  readonly clock?: () => Date;
};

export const LlmDebugLogger = {
  disabled(): LlmDebugLogger {
    return {
      async log(): Promise<void> {
        return undefined;
      },
    };
  },

  createAtPath(path: string, deps: LlmDebugLoggerDeps = {}): LlmDebugLogger {
    const write = deps.appendFile ?? appendFile;
    const ensureDir = deps.mkdir ?? mkdir;
    const now = deps.clock ?? (() => new Date());

    return {
      async log(record: LlmDebugRecord): Promise<void> {
        await ensureDir(dirname(path), { recursive: true });

        const line = JSON.stringify({
          timestamp: now().toISOString(),
          target: record.target,
          provider: record.provider,
          event: record.event,
          fields: redactDiagnosticValue(record.fields ?? {}),
        });

        await write(path, `${line}\n`, 'utf8');
      },
    };
  },
} as const;
