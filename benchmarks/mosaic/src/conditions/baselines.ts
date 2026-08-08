import type { Condition } from '../schemas/index.js';
import { SKILLS, type MicroSkill } from '../catalog/index.js';
import { BASE_TOOL_NAMES, TOOL_NAMES, type ToolName } from '../config/index.js';
import type { JsonValue } from '../core/json.js';
import {
  HarnessInfrastructureError,
  ModelCallBudgetError,
  TOOL_CONTRACTS,
  type EngineResult,
  type EngineUsage,
  type RunContext,
} from '../runtime/index.js';
import type { ConditionPrompts } from './prompts.js';
import { isProviderError, isProviderErrorCode } from './provider.js';

export interface PlannedGoal {
  readonly id: string;
  readonly goal: string;
  readonly doneWhen: readonly string[];
}

export interface BaselinePlan {
  readonly goals: readonly PlannedGoal[];
}

export interface ToolRequest {
  readonly name: string;
  readonly input: JsonValue;
}

export interface BaselineDecision {
  readonly status: 'completed' | 'failed' | 'needs_revision';
  readonly output: string;
  readonly toolCalls: readonly ToolRequest[];
  readonly revisionReason?: string;
}

export interface ModelAnswer<Value> {
  readonly value: Value;
  readonly usage: EngineUsage;
}

export interface BaselineModel {
  readonly plan: (request: {
    readonly system: string;
    readonly user: string;
  }) => Promise<ModelAnswer<BaselinePlan>>;
  readonly execute: (request: {
    readonly system: string;
    readonly user: string;
    readonly toolNames: readonly ToolName[];
  }) => Promise<ModelAnswer<BaselineDecision>>;
  readonly revise: (request: {
    readonly system: string;
    readonly user: string;
  }) => Promise<ModelAnswer<PlannedGoal>>;
}

interface GoalResult {
  readonly status: BaselineDecision['status'];
  readonly output: string;
  readonly toolNames: readonly string[];
  readonly revisionReason?: string;
}

interface ToolObservation {
  readonly name: string;
  readonly input: JsonValue;
  readonly output: JsonValue;
}

const MAX_BASELINE_TURNS = 8;

const emptyUsage = (): EngineUsage => ({
  inputTokens: 0,
  outputTokens: 0,
  costUsd: 0,
});

const addUsage = (left: EngineUsage, right: EngineUsage): EngineUsage => ({
  inputTokens: left.inputTokens + right.inputTokens,
  outputTokens: left.outputTokens + right.outputTokens,
  cachedInputTokens:
    (left.cachedInputTokens ?? 0) + (right.cachedInputTokens ?? 0),
  reasoningTokens: (left.reasoningTokens ?? 0) + (right.reasoningTokens ?? 0),
  costUsd: left.costUsd + right.costUsd,
});

const jsonValue = (value: unknown): JsonValue =>
  JSON.parse(JSON.stringify(value)) as JsonValue;

const callModel = async <Value>(
  context: RunContext,
  stage: 'plan' | 'execution' | 'revision',
  operation: 'plan' | 'execute' | 'revise',
  request: {
    readonly system: string;
    readonly user: string;
    readonly toolNames?: readonly ToolName[];
  },
  invoke: () => Promise<ModelAnswer<Value>>,
): Promise<ModelAnswer<Value>> => {
  await context.startModelCall();
  await context.emit({
    type: 'model.request',
    stage,
    operation,
    model: context.run.model.model,
    messageCount: 2,
    ...(request.toolNames === undefined
      ? {}
      : { toolNames: request.toolNames }),
    prompt: { system: request.system, user: request.user },
  });
  const started = Date.now();
  let answer: ModelAnswer<Value>;
  try {
    answer = await invoke();
  } catch (error) {
    if (isProviderErrorCode(error, 'invalid_structured_output')) {
      await context.emit({
        type: 'structured.attempt',
        stage,
        attempt: 1,
        runtimeAccepted: false,
        feedbackSent: false,
        diagnostic: 'invalid_structured_output',
      });
    }
    await context.emit({
      type: 'model.failed',
      stage,
      operation,
      model: context.run.model.model,
      status: 'failed',
      durationMs: Math.max(0, Date.now() - started),
    });
    if (
      error instanceof HarnessInfrastructureError ||
      error instanceof ModelCallBudgetError
    ) {
      throw error;
    }
    if (isProviderError(error)) {
      throw new HarnessInfrastructureError('model', 'provider_failed');
    }
    throw error;
  }
  const durationMs = Math.max(0, Date.now() - started);
  await context.emit({
    type: 'structured.attempt',
    stage,
    attempt: 1,
    runtimeAccepted: true,
    feedbackSent: false,
  });
  await context.emit({
    type: 'model.response',
    stage,
    operation,
    model: context.run.model.model,
    finishReason: 'structured',
    durationMs,
    response: jsonValue(answer.value),
  });
  return answer;
};

const compare = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;

const tokens = (value: string): ReadonlySet<string> =>
  new Set(value.toLocaleLowerCase('en-US').match(/[a-z0-9]+/gu) ?? []);

const skillScore = (query: ReadonlySet<string>, skill: MicroSkill): number => {
  const body = tokens(`${skill.title} ${skill.description} ${skill.body}`);
  return [...query].reduce(
    (score, token) => score + (body.has(token) ? 1 : 0),
    0,
  );
};

/** Deterministic body-aware catalog ranking used by non-MOSAIC baselines. */
export const rankSkills = (
  query: string,
  limit: number,
): readonly MicroSkill[] => {
  const queryTokens = tokens(query);
  return [...SKILLS]
    .sort(
      (left, right) =>
        skillScore(queryTokens, right) - skillScore(queryTokens, left) ||
        compare(left.id, right.id),
    )
    .slice(0, limit);
};

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

const skillsFor = (
  condition: Condition,
  goal: PlannedGoal,
  request: string,
): readonly MicroSkill[] => {
  if (condition.id === 'B0') return [];
  if (condition.id === 'B1') return rankSkills(request, 1);
  return rankSkills(
    `${request}\n${goal.goal}\n${goal.doneWhen.join('\n')}`,
    condition.id === 'B2' ? 1 : 3,
  );
};

const runGoal = async (
  context: RunContext,
  model: BaselineModel,
  prompts: ConditionPrompts,
  goal: PlannedGoal,
  previous: readonly GoalResult[],
  usage: EngineUsage,
  revision: number,
): Promise<{ readonly result: GoalResult; readonly usage: EngineUsage }> => {
  const skills = skillsFor(
    context.condition,
    goal,
    context.benchmarkCase.request,
  );
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
  for (let turn = 1; turn <= MAX_BASELINE_TURNS; turn += 1) {
    const user = executionPrompt(
      context,
      goal,
      skills,
      tools,
      previous,
      observations,
    );
    const answer = await callModel(
      context,
      'execution',
      'execute',
      { system, user, toolNames: tools },
      () => model.execute({ system, user, toolNames: tools }),
    );
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
  throw new TypeError('baseline model exceeded the bounded tool-evidence loop');
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
  const answer = await callModel(
    context,
    'plan',
    'plan',
    { system: prompts.planner, user },
    () => model.plan({ system: prompts.planner, user }),
  );
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
  return callModel(
    context,
    'revision',
    'revise',
    { system: prompts.revision, user },
    () => model.revise({ system: prompts.revision, user }),
  );
};

/** Executes a complete B0-B3 condition against an injected structured model. */
export const executeBaseline = async (
  context: RunContext,
  model: BaselineModel,
  prompts: ConditionPrompts,
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
