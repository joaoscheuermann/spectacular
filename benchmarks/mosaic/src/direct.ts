import { createAgent, type Agent, type AgentOptions } from 'agent';
import type { Skill } from 'bundle';
import { createMessageStorage } from 'messages';
import { homedir } from 'node:os';
import { createToolStorage, type Tool } from 'tool';

import { directEvent } from './events.js';
import { createProvider, type ProviderProfile, type Runner } from './run.js';
import { loadSkills } from './skills.js';
import { createTerminal } from './terminal.js';

export interface DirectDependencies {
  readonly profile?: ProviderProfile;
  readonly home?: string;
  readonly loadSkills?: (home: string) => Promise<readonly Skill[]>;
  readonly createTerminal?: (options: {
    readonly cwd: string;
    readonly signal?: AbortSignal;
  }) => Tool;
  readonly createAgent?: (options: AgentOptions) => Agent;
}

/** Creates the direct benchmark condition with fresh prompt-local state. */
export const direct = (dependencies: DirectDependencies = {}): Runner => {
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
      const agent = (dependencies.createAgent ?? createAgent)({
        provider: profile.provider,
        model: profile.model,
        effort: 'low',
        system: systemPrompt(skills),
        messages: createMessageStorage(),
        tools: createToolStorage([terminal]),
      });

      for await (const event of agent.stream(request.prompt, {
        maxTurns: 32,
        signal: request.signal,
      })) {
        const normalized = directEvent(event);
        if (normalized !== undefined) await emit(normalized);
      }
    },
  };
};

const systemPrompt = (skills: readonly Skill[]): string =>
  [
    '# Outcome',
    '',
    "Complete the user's request in the current working directory.",
    '',
    '# Constraints',
    '',
    '- Use the terminal when repository evidence or changes are needed.',
    '- Keep the final response concise and evidence-based.',
    ...skills.flatMap((skill) => [
      '',
      `## Skill: ${skill.name}`,
      '',
      skill.body,
    ]),
  ].join('\n');
