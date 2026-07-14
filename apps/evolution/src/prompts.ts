import type { FailedEval } from './evaluate.js';
import type { OptimizerHistory } from './history.js';
import type { EvalAssertion, Scenario } from './schema.js';

export const optimizerSystemPrompt = [
  'Improve one system prompt against the supplied training assertions.',
  'Return a complete replacement prompt, not a patch.',
  'Use only the training scenarios, current failures, and matching history supplied.',
  'Do not return a prompt equivalent to the incumbent or any previously attempted prompt.',
  'Do not reuse a previously failed strategy.',
  'Preserve the original task intent while correcting observed failures.',
  'Return only JSON with prompt:string and strategy:string.',
  'Do not wrap the JSON in Markdown.',
].join(' ');

export const compressionSystemPrompt = [
  'Compress the supplied system prompt without changing its behavior.',
  'Return a complete prompt that is 20 to 30 percent shorter by character count.',
  'Use only the supplied training contract and never weaken an assertion.',
  'Return only JSON with prompt:string and strategy:string.',
  'Do not wrap the JSON in Markdown.',
].join(' ');

export const resultJudgeSystemPrompt = [
  'Judge the supplied model output against the supplied assertion.',
  'Use the supplied scenario input as context.',
  'Return exactly one verdict.',
  'Set passed only when the output clearly satisfies the assertion.',
  'Give concise evidence-based reasoning.',
  'Do not return the eval id or sample index; the caller already knows them.',
  'Return only JSON with reasoning:string and passed:boolean.',
  'Do not wrap the JSON in Markdown.',
].join(' ');

type OptimizerInputs = {
  readonly prompt: string;
  readonly trainingScenarios: readonly Scenario[];
  readonly currentFailures: readonly FailedEval[];
  readonly history: readonly OptimizerHistory[];
  readonly prohibitedPrompts: readonly string[];
  readonly prohibitedStrategies: readonly string[];
};

type CompressionInputs = Pick<
  OptimizerInputs,
  'prompt' | 'trainingScenarios'
>;

type JudgeInputs = {
  readonly scenarioInput: string;
  readonly assertion: EvalAssertion;
  readonly sampleIndex: number;
  readonly modelOutput: string;
};

const longestRun = (body: string, marker: '`' | '~'): number => {
  let longest = 0;
  let current = 0;
  for (const character of body) {
    current = character === marker ? current + 1 : 0;
    longest = Math.max(longest, current);
  }
  return longest;
};

const fenced = (body: string): string => {
  const backticks = Math.max(3, longestRun(body, '`') + 1);
  const tildes = Math.max(3, longestRun(body, '~') + 1);
  const marker = backticks <= tildes ? '`' : '~';
  const delimiter = marker.repeat(marker === '`' ? backticks : tildes);
  const closingSeparator = body.endsWith('\n') ? '' : '\n';
  return `${delimiter}\n${body}${closingSeparator}${delimiter}`;
};

const field = (label: string, value: string): string =>
  `${label}:\n\n${fenced(value)}`;

const collection = (items: readonly string[]): string =>
  items.length === 0 ? 'None.' : items.join('\n\n');

const assertion = (value: EvalAssertion, index: number): string =>
  [
    `### Assertion ${index + 1}`,
    field('ID', value.id),
    field('Text', value.assertion),
  ].join('\n\n');

const scenario = (value: Scenario, index: number): string => {
  const parts = [
    `## Scenario ${index + 1}`,
    field('ID', value.id),
    field('Split', value.split),
    field('Input', value.input),
    `Assertions:\n\n${collection(value.evals.map(assertion))}`,
  ];
  if (value.rationale !== undefined) {
    parts.push(field('Rationale', value.rationale));
  }
  return parts.join('\n\n');
};

const scenarios = (values: readonly Scenario[]): string =>
  collection(values.map(scenario));

const failure = (
  value: FailedEval,
  index: number,
  heading = '##',
): string =>
  [
    `${heading} Failure ${index + 1}`,
    field('Scenario ID', value.scenarioId),
    field('Eval ID', value.evalId),
    `Sample Index: ${value.sampleIndex}`,
    `Passed: ${value.passed}`,
    field('Output', value.output),
    field('Reasoning', value.reasoning),
  ].join('\n\n');

const failures = (values: readonly FailedEval[], heading = '##'): string =>
  collection(values.map((value, index) => failure(value, index, heading)));

const attempt = (value: OptimizerHistory, index: number): string =>
  [
    `## Attempt ${index + 1}`,
    field('Attempted Prompt', value.attemptedPrompt),
    field('Strategy', value.strategy),
    field('Optimizer Mode', value.optimizerMode),
    `Training Accuracy: ${value.trainingAccuracy}`,
    `Failed Evaluations:\n\n${failures(value.failedEvals, '###')}`,
    field('Disposition', value.disposition),
  ].join('\n\n');

const attempts = (values: readonly OptimizerHistory[]): string =>
  collection(values.map(attempt));

const prohibited = (label: string, values: readonly string[]): string =>
  collection(
    values.map((value, index) => `## ${label} ${index + 1}\n\n${fenced(value)}`),
  );

const section = (title: string, body: string): string =>
  `# ${title}\n\n${body}`;

/** Renders the complete optimizer user message as deterministic Markdown. */
export const optimizerInput = (inputs: OptimizerInputs): string =>
  [
    section('Prompt', fenced(inputs.prompt)),
    section('Training Scenarios', scenarios(inputs.trainingScenarios)),
    section('Current Failures', failures(inputs.currentFailures)),
    section('Matching History', attempts(inputs.history)),
    section(
      'Prohibited Prompts',
      prohibited('Prompt', inputs.prohibitedPrompts),
    ),
    section(
      'Prohibited Strategies',
      prohibited('Strategy', inputs.prohibitedStrategies),
    ),
  ].join('\n\n');

/** Renders the complete compression user message as deterministic Markdown. */
export const compressionInput = (inputs: CompressionInputs): string =>
  [
    section('Prompt', fenced(inputs.prompt)),
    section('Training Scenarios', scenarios(inputs.trainingScenarios)),
  ].join('\n\n');

/** Renders one assertion/sample judge user message as deterministic Markdown. */
export const judgeInput = (inputs: JudgeInputs): string =>
  [
    section('Scenario Input', fenced(inputs.scenarioInput)),
    section(
      'Assertion',
      [
        field('ID', inputs.assertion.id),
        field('Text', inputs.assertion.assertion),
      ].join('\n\n'),
    ),
    section('Sample', `Index: ${inputs.sampleIndex}`),
    section('Model Output', fenced(inputs.modelOutput)),
  ].join('\n\n');
