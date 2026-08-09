import type { Condition } from '../schemas/index.js';
import type { MicroSkill } from '../catalog/index.js';
import { BASE_TOOL_NAMES, TOOL_NAMES, type ToolName } from '../config/index.js';
import type { JsonValue } from '../core/json.js';
import {
  TOOL_CONTRACTS,
  type EngineResult,
  type EngineUsage,
  type RunContext,
} from '../runtime/index.js';
import type {
  BaselineDecision,
  BaselineModel,
  BaselinePlan,
  ModelAnswer,
  PlannedGoal,
} from './baseline-contracts.js';
import { callModel } from './baseline-lifecycle.js';
import type { ConditionPrompts } from './prompts.js';
import { isAgentErrorCode } from './provider.js';
import {
  rankedSkills,
  skillDocument,
  type SkillRetrieval,
} from './retrieval.js';

export type {
  BaselineCallLifecycle,
  BaselineDecision,
  BaselineModel,
  BaselinePlan,
  BaselineRunControls,
  ModelAnswer,
  PlannedGoal,
  ToolRequest,
} from './baseline-contracts.js';

interface GoalResult {
  readonly status: BaselineDecision['status'] | 'blocked';
  readonly output: string;
  readonly toolNames: readonly string[];
  readonly revisionReason?: string;
}

interface ToolObservation {
  readonly name: string;
  readonly input: JsonValue;
  readonly output: JsonValue;
}

const emptyUsage = (): EngineUsage => ({
  inputTokens: 0,
  outputTokens: 0,
  costUsd: 0,
});

const optionalSum = (
  left: number | undefined,
  right: number | undefined,
): number | undefined =>
  left === undefined && right === undefined
    ? undefined
    : (left ?? 0) + (right ?? 0);

const addUsage = (left: EngineUsage, right: EngineUsage): EngineUsage => ({
  inputTokens: left.inputTokens + right.inputTokens,
  outputTokens: left.outputTokens + right.outputTokens,
  cachedInputTokens: optionalSum(
    left.cachedInputTokens,
    right.cachedInputTokens,
  ),
  reasoningTokens: optionalSum(left.reasoningTokens, right.reasoningTokens),
  costUsd: left.costUsd + right.costUsd,
});

const fence = (value: string): string => {
  const longest = Math.max(
    0,
    ...(value.match(/`+/gu) ?? []).map((item) => item.length),
  );
  const marker = '`'.repeat(Math.max(3, longest + 1));
  return `${marker}text\n${value}\n${marker}`;
};

const skillSection = (skills: readonly MicroSkill[]): string =>
  skills.length === 0
    ? 'No skill is available.'
    : skills
        .map((skill) => `## ${skill.title}\n\n${fence(skill.body)}`)
        .join('\n\n');

const toolSection = (tools: readonly ToolName[]): string =>
  tools.length === 0
    ? 'No tool is available.'
    : tools
        .map((tool) => `- ${tool}: ${TOOL_CONTRACTS[tool].description}`)
        .join('\n');

const observationsSection = (
  observations: readonly ToolObservation[],
): string =>
  observations.length === 0
    ? 'No tool observation is available yet.'
    : observations
        .map(
          (observation, index) =>
            `## Observation ${index + 1}: ${observation.name}\n\nInput:\n\n\`\`\`json\n${JSON.stringify(observation.input, null, 2)}\n\`\`\`\n\nOutput:\n\n\`\`\`json\n${JSON.stringify(observation.output, null, 2)}\n\`\`\``,
        )
        .join('\n\n');

const executionPrompt = (
  context: RunContext,
  goal: PlannedGoal,
  skills: readonly MicroSkill[],
  tools: readonly ToolName[],
  previous: readonly GoalResult[],
  observations: readonly ToolObservation[],
): string =>
  `# Request\n\n${fence(context.benchmarkCase.request)}\n\n# Current goal\n\n${fence(goal.goal)}\n\n## Completion criteria\n\n${goal.doneWhen.map((criterion) => `- ${criterion}`).join('\n')}\n\n# Available skills\n\n${skillSection(skills)}\n\n# Available tools\n\n${toolSection(tools)}\n\n# Prior goal outcomes\n\n${previous.length === 0 ? 'No prior goal outcome.' : previous.map((result) => `- ${result.status}: ${result.output}`).join('\n')}\n\n# Tool observations\n\n${observationsSection(observations)}`;

const planningPrompt = (context: RunContext, mode: 'task' | 'goal'): string =>
  `# Request\n\n${fence(context.benchmarkCase.request)}\n\n# Decomposition\n\nCreate a ${mode}-oriented plan. Derive observable completion criteria only from the explicit request. Do not assume hidden requirements.`;

const validatePlan = (plan: BaselinePlan): readonly PlannedGoal[] => {
  if (!Array.isArray(plan.goals) || plan.goals.length === 0)
    throw new TypeError('baseline planner returned no goals');
  const ids = new Set<string>();
  plan.goals.forEach((goal) => {
    if (
      goal.id.trim() === '' ||
      goal.goal.trim() === '' ||
      goal.doneWhen.length === 0 ||
      ids.has(goal.id)
    ) {
      throw new TypeError('baseline planner returned an invalid goal');
    }
    ids.add(goal.id);
  });
  return plan.goals;
};

const toolsFor = (
  condition: Condition,
  skills: readonly MicroSkill[],
): readonly ToolName[] => {
  if (condition.factors.menu === 'global') return TOOL_NAMES;
  const declared = skills.flatMap((skill) => skill.allowedTools);
  const names = condition.factors.baseTools
    ? [...BASE_TOOL_NAMES, ...declared]
    : declared;
  return [...new Set(names)];
};

const skillQuery = (goal: PlannedGoal, request: string): string =>
  `# Request\n\n${fence(request)}\n\n# Goal\n\n${fence(goal.goal)}\n\n# Completion criteria\n\n${goal.doneWhen.map((criterion) => `- ${criterion}`).join('\n')}`;

const observedRetrieval = async <Value>(
  context: RunContext,
  operation: 'embedding' | 'rerank',
  model: string,
  request: Readonly<Record<string, JsonValue>>,
  invoke: () => Promise<Value>,
): Promise<Value> => {
  await context.startModelCall();
  await context.emit({
    type: 'model.request',
    stage: 'retrieval',
    operation,
    model,
    messageCount: 1,
    ...request,
  });
  const started = Date.now();
  try {
    const value = await invoke();
    await context.emit({
      type: 'model.response',
      stage: 'retrieval',
      operation,
      model,
      durationMs: Math.max(0, Date.now() - started),
    });
    return value;
  } catch (error) {
    await context.emit({
      type: 'model.failed',
      stage: 'retrieval',
      operation,
      model,
      status: 'failed',
      durationMs: Math.max(0, Date.now() - started),
    });
    throw error;
  }
};

const skillsFor = async (
  context: RunContext,
  goal: PlannedGoal,
  revision: number,
  retrieval: SkillRetrieval,
): Promise<readonly MicroSkill[]> => {
  if (context.condition.id === 'B0') return [];
  const maxCandidates = context.condition.factors.maxCandidates;
  const maxSkills = context.condition.factors.maxSkills;
  if (maxCandidates === null || maxSkills === null || maxSkills <= 0)
    throw new TypeError('skill-bearing baseline requires retrieval limits');
  const query =
    context.condition.id === 'B1'
      ? context.benchmarkCase.request
      : skillQuery(goal, context.benchmarkCase.request);
  const matches = await observedRetrieval(
    context,
    'embedding',
    retrieval.models.embedder,
    { query },
    () => retrieval.search(query, maxCandidates),
  );
  await context.emit({
    type: 'retrieval.result',
    stage: 'bundle',
    goalId: goal.id,
    revision,
    k: maxCandidates,
    skillNames: matches.map(({ skill }) => skill.id),
  });
  if (matches.length < maxSkills)
    throw new TypeError('retrieval returned too few skills for the bundle');
  const ranking = await observedRetrieval(
    context,
    'rerank',
    retrieval.models.reranker,
    { count: matches.length },
    () =>
      retrieval.rerank({
        model: retrieval.models.reranker,
        query,
        documents: matches.map(({ skill }) => skillDocument(skill)),
        topN: matches.length,
        flags: { sensitiveOutput: true },
      }),
  );
  const ranked = rankedSkills(matches, ranking);
  await context.emit({
    type: 'rerank.result',
    stage: 'bundle',
    goalId: goal.id,
    revision,
    ranking: ranked.map(({ skill, relevanceScore, rank }) => ({
      skillName: skill.id,
      score: relevanceScore,
      rank,
    })),
  });
  return ranked.slice(0, maxSkills).map(({ skill }) => skill);
};

const runGoal = async (
  context: RunContext,
  model: BaselineModel,
  prompts: ConditionPrompts,
  goal: PlannedGoal,
  previous: readonly GoalResult[],
  usage: EngineUsage,
  revision: number,
  retrieval: SkillRetrieval,
): Promise<{ readonly result: GoalResult; readonly usage: EngineUsage }> => {
  const skills = await skillsFor(context, goal, revision, retrieval);
  const tools = toolsFor(context.condition, skills);
  await context.emit({
    type: 'baseline.menu',
    goalId: goal.id,
    revision,
    skillNames: skills.map((skill) => skill.id),
    toolNames: tools,
  });
  const allowed = new Set(tools);
  const observations: ToolObservation[] = [];
  let currentUsage = usage;
  const system =
    context.condition.id === 'B0' || context.condition.id === 'B1'
      ? prompts.baseline
      : prompts.executor;
  let turns = 0;
  while (turns < context.condition.factors.maxTurns) {
    const user = executionPrompt(
      context,
      goal,
      skills,
      tools,
      previous,
      observations,
    );
    let completion;
    try {
      completion = await callModel(
        context,
        model,
        'execution',
        'execute',
        { system, user, toolNames: tools },
        context.condition.factors.maxTurns - turns,
        (controls) =>
          model.execute({ system, user, toolNames: tools, controls }),
      );
    } catch (error) {
      if (!isAgentErrorCode(error, 'turn_limit_exceeded')) throw error;
      return {
        result: {
          status: 'blocked',
          output: '',
          toolNames: observations.map((observation) => observation.name),
        },
        usage: currentUsage,
      };
    }
    turns += completion.calls;
    const { answer } = completion;
    currentUsage = addUsage(currentUsage, answer.usage);
    for (const call of answer.value.toolCalls) {
      if (!allowed.has(call.name as ToolName))
        throw new TypeError(
          `baseline requested unavailable tool: ${call.name}`,
        );
      const output = await context.callTool(call.name as ToolName, call.input, {
        nodeId: goal.id,
        revision,
      });
      observations.push({ name: call.name, input: call.input, output });
    }
    if (answer.value.toolCalls.length > 0) continue;
    return {
      result: {
        status: answer.value.status,
        output: answer.value.output,
        toolNames: observations.map((observation) => observation.name),
        revisionReason: answer.value.revisionReason,
      },
      usage: currentUsage,
    };
  }
  return {
    result: {
      status: 'blocked',
      output: '',
      toolNames: observations.map((observation) => observation.name),
    },
    usage: currentUsage,
  };
};

const initialGoals = async (
  context: RunContext,
  model: BaselineModel,
  prompts: ConditionPrompts,
): Promise<{
  readonly goals: readonly PlannedGoal[];
  readonly usage: EngineUsage;
}> => {
  if (context.condition.id === 'B0' || context.condition.id === 'B1') {
    return {
      goals: [
        {
          id: 'g01',
          goal: context.benchmarkCase.request,
          doneWhen: ['Every explicit requirement in the request is satisfied.'],
        },
      ],
      usage: emptyUsage(),
    };
  }
  const user = planningPrompt(
    context,
    context.condition.id === 'B2' ? 'task' : 'goal',
  );
  const completion = await callModel(
    context,
    model,
    'plan',
    'plan',
    { system: prompts.planner, user },
    context.condition.factors.maxTurns,
    (controls) => model.plan({ system: prompts.planner, user, controls }),
  );
  const { answer } = completion;
  return { goals: validatePlan(answer.value), usage: answer.usage };
};

const reviseGoal = async (
  context: RunContext,
  model: BaselineModel,
  prompts: ConditionPrompts,
  goal: PlannedGoal,
  result: GoalResult,
): Promise<ModelAnswer<PlannedGoal>> => {
  await context.emit({
    type: 'baseline.revision',
    goalId: goal.id,
    revision: 1,
  });
  const user = `# Request\n\n${fence(context.benchmarkCase.request)}\n\n# Goal requiring revision\n\n${fence(goal.goal)}\n\n# Observed reason\n\n${fence(result.revisionReason ?? 'The goal requested revision without a reason.')}`;
  const completion = await callModel(
    context,
    model,
    'revision',
    'revise',
    { system: prompts.revision, user },
    context.condition.factors.maxTurns,
    (controls) => model.revise({ system: prompts.revision, user, controls }),
  );
  return completion.answer;
};

/** Executes a complete B0-B3 condition against an injected structured model. */
export const executeBaseline = async (
  context: RunContext,
  model: BaselineModel,
  prompts: ConditionPrompts,
  retrieval: SkillRetrieval,
): Promise<EngineResult> => {
  if (!['B0', 'B1', 'B2', 'B3'].includes(context.condition.id))
    throw new TypeError('condition is not a baseline executor');
  const initial = await initialGoals(context, model, prompts);
  let usage = initial.usage;
  const results: GoalResult[] = [];
  for (const goal of initial.goals) {
    let execution = await runGoal(
      context,
      model,
      prompts,
      goal,
      results,
      usage,
      0,
      retrieval,
    );
    usage = execution.usage;
    if (
      execution.result.status === 'needs_revision' &&
      context.condition.factors.localizedRevision
    ) {
      const revised = await reviseGoal(
        context,
        model,
        prompts,
        goal,
        execution.result,
      );
      usage = addUsage(usage, revised.usage);
      execution = await runGoal(
        context,
        model,
        prompts,
        revised.value,
        results,
        usage,
        1,
        retrieval,
      );
      usage = execution.usage;
    }
    results.push(execution.result);
  }
  const failed = results.some((result) => result.status !== 'completed');
  return {
    status: failed ? 'failed' : 'succeeded',
    outcome: {
      conditionId: context.condition.id,
      goals: results.map((result) => ({
        status: result.status,
        output: result.output,
        toolNames: result.toolNames,
      })),
    },
    usage,
  };
};
