import type { Skill } from 'bundle';
import {
  mosaic as createMosaic,
  type MosaicAgent,
  type MosaicOptions,
} from 'mosaic';
import { randomUUID } from 'node:crypto';
import { homedir } from 'node:os';
import pino from 'pino';
import type { Tool } from 'tool';

import { mosaicEvent, mosaicResult } from './events.js';
import { createProvider, type ProviderProfile, type Runner } from './run.js';
import { loadSkills } from './skills.js';
import { createTerminal } from './terminal.js';

export interface MosaicDependencies {
  readonly profile?: ProviderProfile;
  readonly home?: string;
  readonly loadSkills?: (home: string) => Promise<readonly Skill[]>;
  readonly createTerminal?: (options: {
    readonly cwd: string;
    readonly signal?: AbortSignal;
  }) => Tool;
  readonly createWorkflow?: (options: MosaicOptions) => MosaicAgent;
  readonly randomUUID?: () => string;
}

/** Creates the MOSAIC benchmark condition with public IO observability. */
export const mosaic = (dependencies: MosaicDependencies = {}): Runner => {
  const profile = dependencies.profile ?? createProvider();

  return {
    async run(request, emit): Promise<void> {
      const skills = await (dependencies.loadSkills ?? loadSkills)(
        dependencies.home ?? homedir(),
      );
      const terminal = (dependencies.createTerminal ?? createTerminal)({
        cwd: request.cwd,
        signal: request.signal,
      });
      const workflow = (dependencies.createWorkflow ?? createMosaic)(
        options(profile, skills, terminal),
      );
      const result = await workflow.prompt(request.prompt, {
        runId: (dependencies.randomUUID ?? randomUUID)(),
        signal: request.signal,
        capture: 'io',
        observer: async (event) => {
          const normalized = mosaicEvent(event);
          if (normalized !== undefined) await emit(normalized);
        },
      });
      const delivery = mosaicResult(result);
      if (delivery !== undefined) await emit(delivery);
    },
  };
};

const options = (
  profile: ProviderProfile,
  skills: readonly Skill[],
  terminal: Tool,
): MosaicOptions => ({
  logger: pino({ enabled: false }),
  providers: {
    planning: profile.provider,
    revision: profile.provider,
    execution: profile.provider,
    reranker: profile.provider,
  },
  models: {
    planning: { model: profile.model, effort: 'low' },
    revision: { model: profile.model, effort: 'low' },
    execution: { model: profile.model, effort: 'low' },
    reranker: profile.model,
    embedder: profile.model,
  },
  routing: {
    maxHintCandidates: 1,
    maxRetrievedCandidates: 1,
    maxSkills: 0,
  },
  revision: { max: 3 },
  execution: { maxTurns: 32 },
  skills: { required: skills, menu: skills, retriever: noSearch },
  tools: { required: [terminal], menu: [terminal], retriever: noSearch },
});

const noSearch = {
  search: async (): Promise<readonly never[]> => [],
};
