import { z } from 'zod';
import type { MosaicEvent, MosaicOptions } from 'mosaic';
import type {
  ExecutionResult,
  LocalizedRevisionInput,
  MosaicEvaluationHooks,
} from 'mosaic/evaluation';

import type { Case } from '../schemas/index.js';
import { SKILLS } from '../catalog/index.js';
import { BASE_TOOL_NAMES, TOOL_NAMES } from '../config/index.js';
import type { JsonValue } from '../core/json.js';
import {
  TOOL_CONTRACTS,
  type EngineUsage,
  type RunContext,
  type ToolCallCorrelation,
} from '../runtime/index.js';

type MosaicSkill = MosaicOptions['skills']['menu'][number];
type MosaicTool = MosaicOptions['tools']['menu'][number];

export type MosaicBaseOptions = Pick<
  MosaicOptions,
  'logger' | 'provider' | 'models' | 'routing' | 'execution' | 'revision'
>;

export interface MosaicOracleDependencies {
  readonly state?: (
    input: Parameters<NonNullable<MosaicEvaluationHooks['execution']>>[0],
    benchmarkCase: Case,
    context: RunContext,
  ) => Promise<ExecutionResult>;
  readonly revision?: (
    input: Readonly<LocalizedRevisionInput>,
    benchmarkCase: Case,
  ) => Promise<
    Awaited<
      ReturnType<
        Parameters<NonNullable<MosaicEvaluationHooks['localizedRevision']>>[1]
      >
    >
  >;
}

export interface MosaicDependencies {
  readonly base: MosaicBaseOptions;
  readonly retrievers: {
    readonly skills: MosaicOptions['skills']['retriever'];
    readonly metadataSkills: MosaicOptions['skills']['retriever'];
    readonly tools: MosaicOptions['tools']['retriever'];
  };
  readonly retrieverModels?: {
    readonly skills?: string;
    readonly metadataSkills?: string;
    readonly tools?: string;
  };
  readonly factory?: typeof import('mosaic/evaluation').mosaic;
  readonly usage?: () => EngineUsage;
  readonly oracles?: MosaicOracleDependencies;
}

export interface ToolCorrelations {
  readonly started: (
    event: Extract<MosaicEvent, { readonly type: 'tool.started' }>,
  ) => void;
  readonly claim: (toolName: string) => ToolCallCorrelation | undefined;
}

/** Correlates MOSAIC tool lifecycle events with the benchmark World adapter. */
export const createToolCorrelations = (): ToolCorrelations => {
  const pending = new Map<string, ToolCallCorrelation[]>();
  return {
    started: ({ callId, nodeId, revision, toolName }) => {
      const values = pending.get(toolName) ?? [];
      values.push({ callId, nodeId, revision });
      pending.set(toolName, values);
    },
    claim: (toolName) => {
      const values = pending.get(toolName);
      const correlation = values?.shift();
      if (values?.length === 0) pending.delete(toolName);
      return correlation;
    },
  };
};

const compare = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;
const terms = (value: string): ReadonlySet<string> =>
  new Set(value.toLocaleLowerCase('en-US').match(/[a-z0-9]+/gu) ?? []);
const overlap = (query: string, value: string): number => {
  const queryTerms = terms(query);
  const valueTerms = terms(value);
  return [...queryTerms].reduce(
    (sum, term) => sum + (valueTerms.has(term) ? 1 : 0),
    0,
  );
};

/** Converts the frozen catalog into canonical MOSAIC skill values. */
export const mosaicSkills = (): readonly MosaicSkill[] =>
  SKILLS.map((skill) => ({
    name: skill.id,
    description: skill.description,
    body: skill.body,
    allowedTools: [...skill.allowedTools],
    indexText: `${skill.id} | ${skill.description} | ${skill.allowedTools.join(',')} | ${skill.body}`,
  }));

/** Binds public MOSAIC tools to the run-local in-memory World adapter. */
export const mosaicTools = (
  context: RunContext,
  correlations?: ToolCorrelations,
): readonly MosaicTool[] =>
  TOOL_NAMES.map((name) => {
    const contract = TOOL_CONTRACTS[name];
    return {
      name,
      description: contract.description,
      input: contract.input,
      output: contract.output,
      definition: {
        name,
        description: contract.description,
        inputSchema: z.toJSONSchema(contract.input, { io: 'output' }),
        outputSchema: z.toJSONSchema(contract.output, {
          io: 'output',
          unrepresentable: 'any',
        }),
        strict: true,
      },
      execute: async (payload) =>
        context.callTool(name, payload as JsonValue, correlations?.claim(name)),
    } as MosaicTool;
  });

const search = <Value>(
  values: readonly Value[],
  text: (value: Value) => string,
) => ({
  search: async (query: string, topK: number) =>
    [...values]
      .map((data) => ({ data, score: overlap(query, text(data)) }))
      .sort(
        (left, right) =>
          right.score - left.score ||
          compare(text(left.data), text(right.data)),
      )
      .slice(0, topK),
});

/** Test-only lexical retrievers; empirical runs must inject the frozen hybrid family. */
export const createTestLexicalRetrievers = () => {
  const skills = mosaicSkills();
  const tools = TOOL_NAMES.map((name) => ({
    name,
    description: TOOL_CONTRACTS[name].description,
  })) as unknown as readonly MosaicTool[];
  return {
    skills: search(skills, (skill) => skill.indexText),
    metadataSkills: search(
      skills,
      (skill) =>
        `${skill.name} ${skill.description} ${skill.allowedTools.join(' ')}`,
    ),
    tools: search(tools, (tool) => `${tool.name} ${tool.description ?? ''}`),
  };
};

/** Composes validated public MOSAIC options for one experimental condition. */
export const mosaicOptions = (
  context: RunContext,
  dependencies: MosaicDependencies,
  correlations?: ToolCorrelations,
): MosaicOptions => {
  const { base } = dependencies;
  const skills = mosaicSkills();
  const tools = mosaicTools(context, correlations);
  const byName = new Map(tools.map((tool) => [tool.name, tool]));
  const required = context.condition.factors.baseTools
    ? BASE_TOOL_NAMES.map((name) => byName.get(name)).filter(
        (tool): tool is MosaicTool => tool !== undefined,
      )
    : [];
  return {
    ...base,
    routing: {
      ...base.routing,
      maxSkills: context.condition.factors.maxSkills ?? base.routing.maxSkills,
    },
    revision: {
      max: context.condition.factors.localizedRevision ? base.revision.max : 0,
    },
    skills: {
      required: [],
      menu: skills,
      retriever:
        context.condition.id === 'A1'
          ? dependencies.retrievers.metadataSkills
          : dependencies.retrievers.skills,
    },
    tools: { required, menu: tools, retriever: dependencies.retrievers.tools },
  };
};
