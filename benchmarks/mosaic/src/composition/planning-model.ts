import { createHash } from 'node:crypto';

import { createAgent, createToolCallStorage } from 'agent';
import type {
  JsonValue,
  LlmProvider,
  ProviderRequest,
  ReasoningEffort,
  StructuredOutputSchema,
  StructuredOutputValue,
} from 'llms';
import { createMessageStorage } from 'messages';
import { createToolStorage } from 'tool';

import { PlanningGraphSchema, type PlanningGraph } from './planning-schema.js';
import { planningObservationSchema } from './planning-observation-schema.js';
import type {
  PlanningObservationInput,
  PlanningCondition,
  PlanningRevisionInput,
  PlanningRunAdapter,
  PlanningSkill,
} from './planning-runner.js';

export type PlanningModelOperation =
  | 'initial-plan'
  | 'revision'
  | 'observation';

export interface PlanningLexicalMatch {
  readonly skillId: string;
  readonly score: number;
  readonly matchedTerms: readonly string[];
}

export interface PlanningRetrievalSource {
  readonly request: string;
  readonly catalog: readonly Pick<
    PlanningSkill,
    'id' | 'name' | 'description' | 'body' | 'toolIds'
  >[];
}

export type PlanningModelEvent =
  | {
      readonly type: 'cache.hit';
      readonly operation: PlanningModelOperation;
      readonly caseId: string;
      readonly condition: PlanningCondition | null;
    }
  | {
      readonly type: 'model.call';
      readonly operation: PlanningModelOperation;
      readonly caseId: string;
      readonly condition: PlanningCondition | null;
      readonly call: number;
    }
  | {
      readonly type: 'structured.attempt';
      readonly operation: PlanningModelOperation;
      readonly caseId: string;
      readonly condition: PlanningCondition | null;
      readonly attempt: number;
      readonly runtimeAccepted: boolean;
      readonly feedbackSent: boolean;
      readonly diagnostic?: string;
    }
  | {
      readonly type: 'model.completed';
      readonly operation: PlanningModelOperation;
      readonly caseId: string;
      readonly condition: PlanningCondition | null;
      readonly calls: number;
    }
  | {
      readonly type: 'retrieval.completed';
      readonly caseId: string;
      readonly condition: 'retrieved';
      readonly matches: readonly PlanningLexicalMatch[];
    };

export interface PlanningModelAdapterOptions {
  readonly provider: LlmProvider;
  readonly model: string;
  readonly effort?: ReasoningEffort;
  /** Bounds provider turns, including structured-output repairs. Defaults to 3. */
  readonly maxTurns?: number;
  readonly signal?: AbortSignal;
  /** Receives one awaited, body-free event for every real call or cache result. */
  readonly onEvent?: (event: PlanningModelEvent) => void | Promise<void>;
}

const defaultMaxTurns = 3;

const initialSystem = [
  'Create the smallest outcome-oriented initial plan P0 for the request.',
  '',
  '# Requirements',
  '',
  '- Use only the request. Do not assume or request a skill catalog.',
  '- Preserve every requested constraint and user-facing deliverable.',
  '- Describe observable results rather than tools or implementation steps.',
  '- Split nodes only for genuine prerequisites or independently produced results.',
  '- Give every node a stable unique ID and observable completion criteria.',
  '- Keep dependencies existing, unique, and acyclic.',
  '- Mark only terminal user-facing results for delivery.',
  '- Return the complete plan through the structured output tool.',
].join('\n');

const revisionSystem = [
  'Produce the complete revised plan P1 from P0 and the supplied skill evidence.',
  '',
  '# Requirements',
  '',
  '- Preserve the request intent, constraints, deliverables, and valid P0 content.',
  '- Apply only instructions supported by the supplied skill evidence.',
  '- Change structure or completion criteria only when the evidence requires it.',
  '- Do not mention skill identities in plan content unless the request requires it.',
  '- Keep node IDs stable when their semantic responsibility remains stable.',
  '- Keep dependencies existing, unique, and acyclic.',
  '- Return the complete revised plan through the structured output tool.',
].join('\n');

const observationSystem = [
  'Classify the semantic content of a plan using the supplied label catalog.',
  '',
  '# Requirements',
  '',
  '- Return exactly one observation node for every plan node, in plan order.',
  '- Copy each plan node ID and dependency ID exactly.',
  '- Assign at least one best-matching role to every node.',
  '- Assign output and behavior IDs only when the node explicitly states or',
  '  unambiguously entails that label.',
  '- Use only IDs from the supplied catalog and never invent labels.',
  '- Judge the plan text itself; do not infer content from the benchmark phase.',
  '- Return the classification through the structured output tool.',
].join('\n');

/**
 * Creates a run adapter whose three semantic operations use isolated public
 * Agent instances. Successful structured results are cached per adapter.
 */
export const createPlanningModelAdapter = (
  options: PlanningModelAdapterOptions,
): PlanningRunAdapter => {
  const model = options.model.trim();
  const maxTurns = options.maxTurns ?? defaultMaxTurns;
  if (model.length === 0) {
    throw new TypeError('Planning model must be non-empty.');
  }
  if (!Number.isSafeInteger(maxTurns) || maxTurns <= 0) {
    throw new TypeError(
      'Planning model maxTurns must be a positive safe integer.',
    );
  }

  const cache = new Map<string, unknown>();
  const emit = async (event: PlanningModelEvent): Promise<void> => {
    await options.onEvent?.(event);
  };

  const complete = async <Schema extends StructuredOutputSchema>({
    operation,
    caseId,
    condition,
    system,
    input,
    schema,
  }: ModelCompletion<Schema>): Promise<StructuredOutputValue<Schema>> => {
    const key = cacheKey(operation, system, input);
    if (cache.has(key)) {
      await emit({ type: 'cache.hit', operation, caseId, condition });
      return schema.parse(cache.get(key));
    }

    let calls = 0;
    const provider = trackedProvider(options.provider, async () => {
      calls += 1;
      await emit({
        type: 'model.call',
        operation,
        caseId,
        condition,
        call: calls,
      });
    });
    const agent = createAgent({
      provider,
      model,
      system,
      messages: createMessageStorage(),
      toolCalls: createToolCallStorage(),
      tools: createToolStorage([]),
      ...(options.effort === undefined ? {} : { effort: options.effort }),
    });
    const response = await agent.complete(input, {
      schema,
      maxTurns,
      ...(options.signal === undefined ? {} : { signal: options.signal }),
      onStructuredAttempt: (event) =>
        emit({
          type: 'structured.attempt',
          operation,
          caseId,
          condition,
          attempt: event.attempt,
          runtimeAccepted: event.runtimeAccepted,
          feedbackSent: event.feedbackSent,
          ...(event.diagnostic === undefined
            ? {}
            : { diagnostic: event.diagnostic }),
        }),
    });
    const output = schema.parse(response.structured);
    cache.set(key, output);
    await emit({
      type: 'model.completed',
      operation,
      caseId,
      condition,
      calls,
    });
    return output;
  };

  return {
    initialPlan: (benchmarkCase) =>
      complete({
        operation: 'initial-plan',
        caseId: benchmarkCase.id,
        condition: null,
        system: initialSystem,
        input: initialPrompt(benchmarkCase.request),
        schema: PlanningGraphSchema,
      }),
    revise: (input) =>
      complete({
        operation: 'revision',
        caseId: input.case.id,
        condition: input.condition,
        system: revisionSystem,
        input: revisionPrompt(input),
        schema: PlanningGraphSchema,
      }),
    observe: (input) =>
      complete({
        operation: 'observation',
        caseId: input.case.id,
        condition: input.condition,
        system: observationSystem,
        input: observationPrompt(input),
        schema: planningObservationSchema(input.case),
      }),
    retrieve: async ({ case: benchmarkCase, p0 }) => {
      const matches = retrievePlanningSkills(
        {
          request: benchmarkCase.request,
          catalog: benchmarkCase.catalog,
        },
        p0,
      );
      await emit({
        type: 'retrieval.completed',
        caseId: benchmarkCase.id,
        condition: 'retrieved',
        matches,
      });
      return matches.map(({ skillId }) => skillId);
    },
  };
};

/**
 * Ranks every positive lexical skill match. The source type deliberately has
 * no oracle fields, composition class, or expected result cardinality.
 */
export const retrievePlanningSkills = (
  source: PlanningRetrievalSource,
  graph: PlanningGraph,
): readonly PlanningLexicalMatch[] => {
  const plan = PlanningGraphSchema.parse(graph);
  const queryTerms = new Set(
    terms(
      [
        source.request,
        ...plan.nodes.flatMap(({ goal, doneWhen }) => [goal, ...doneWhen]),
      ].join('\n'),
    ),
  );
  const documents = source.catalog.map((skill) => ({
    skillId: skill.id,
    weights: skillTermWeights(skill),
  }));
  const frequencies = documentFrequencies(documents);
  const count = documents.length;

  return documents
    .map(({ skillId, weights }) => {
      const matchedTerms = [...weights.keys()]
        .filter((term) => queryTerms.has(term))
        .sort(compareText);
      const score = matchedTerms.reduce((total, term) => {
        const frequency = frequencies.get(term) ?? count;
        const inverseFrequency = Math.log((count + 1) / (frequency + 1)) + 1;
        return total + (weights.get(term) ?? 0) * inverseFrequency;
      }, 0);
      return {
        skillId,
        score: rounded(score),
        matchedTerms,
      };
    })
    .filter(({ score }) => score > 0)
    .sort(
      (left, right) =>
        right.score - left.score || compareText(left.skillId, right.skillId),
    );
};

type ModelCompletion<Schema extends StructuredOutputSchema> = {
  readonly operation: PlanningModelOperation;
  readonly caseId: string;
  readonly condition: PlanningCondition | null;
  readonly system: string;
  readonly input: string;
  readonly schema: Schema;
};

const initialPrompt = (request: string): string =>
  ['# Initial planning input', '', '## Request', '', fenced(request)].join(
    '\n',
  );

const revisionPrompt = ({
  case: benchmarkCase,
  p0,
  evidence,
}: PlanningRevisionInput): string =>
  [
    '# Plan revision input',
    '',
    '## Request',
    '',
    fenced(benchmarkCase.request),
    '',
    '## Current P0',
    '',
    fenced(renderGraph(PlanningGraphSchema.parse(p0))),
    '',
    '## Skill evidence',
    '',
    evidence.length === 0
      ? 'No skill evidence was supplied.'
      : evidence.map(renderSkill).join('\n\n'),
  ].join('\n');

const observationPrompt = ({
  case: benchmarkCase,
  graph,
}: PlanningObservationInput): string =>
  [
    '# Semantic observation input',
    '',
    '## Request',
    '',
    fenced(benchmarkCase.request),
    '',
    '## Plan',
    '',
    fenced(renderGraph(PlanningGraphSchema.parse(graph))),
    '',
    '## Semantic label catalog',
    '',
    '### Roles',
    '',
    fenced(renderLabels(benchmarkCase.gold.roles)),
    '',
    '### Outputs',
    '',
    fenced(renderLabels(benchmarkCase.gold.outputs)),
    '',
    '### Behaviors',
    '',
    fenced(renderLabels(benchmarkCase.gold.behaviors)),
  ].join('\n');

const renderGraph = (graph: PlanningGraph): string =>
  graph.nodes
    .map((node, index) =>
      [
        `Node ${index + 1}`,
        `ID: ${node.id}`,
        `Goal: ${node.goal}`,
        'Completion criteria:',
        ...node.doneWhen.map(
          (criterion, criterionIndex) => `${criterionIndex + 1}. ${criterion}`,
        ),
        'Dependencies:',
        ...(node.dependsOn.length === 0
          ? ['- None']
          : node.dependsOn.map((id) => `- ${id}`)),
        `Deliver: ${node.deliver ? 'yes' : 'no'}`,
      ].join('\n'),
    )
    .join('\n\n');

const renderSkill = (skill: PlanningSkill, index: number): string =>
  [
    `### Evidence ${index + 1}: ${skill.name}`,
    '',
    `Opaque ID: \`${skill.id}\``,
    '',
    '#### Description',
    '',
    fenced(skill.description),
    '',
    '#### Instructions',
    '',
    fenced(skill.body),
    '',
    '#### Declared tools',
    '',
    skill.toolIds.length === 0
      ? '- None'
      : skill.toolIds.map((id) => `- \`${id}\``).join('\n'),
  ].join('\n');

const renderLabels = (
  values: readonly { readonly id: string; readonly description: string }[],
): string =>
  values.map(({ id, description }) => `${id}\n  ${description}`).join('\n\n');

const fenced = (value: string): string => {
  const longest = [...value.matchAll(/`+/gu)].reduce(
    (length, match) => Math.max(length, match[0].length),
    0,
  );
  const delimiter = '`'.repeat(Math.max(3, longest + 1));
  return `${delimiter}text\n${value}\n${delimiter}`;
};

const skillTermWeights = (
  skill: PlanningRetrievalSource['catalog'][number],
): ReadonlyMap<string, number> => {
  const weights = new Map<string, number>();
  for (const [value, weight] of [
    [skill.name, 4],
    [skill.description, 2],
    [skill.body, 1],
    [skill.toolIds.join(' '), 2],
  ] as const) {
    terms(value).forEach((term) =>
      weights.set(term, Math.max(weight, weights.get(term) ?? 0)),
    );
  }
  return weights;
};

const documentFrequencies = (
  documents: readonly { readonly weights: ReadonlyMap<string, number> }[],
): ReadonlyMap<string, number> => {
  const frequencies = new Map<string, number>();
  documents.forEach(({ weights }) =>
    weights.forEach((_, term) =>
      frequencies.set(term, (frequencies.get(term) ?? 0) + 1),
    ),
  );
  return frequencies;
};

const stopWords = new Set(
  'and are for from into not only that the their then this use using with'.split(
    ' ',
  ),
);

const terms = (value: string): readonly string[] =>
  [
    ...value
      .normalize('NFKC')
      .toLowerCase()
      .matchAll(/[\p{L}\p{N}]+/gu),
  ]
    .map(([term]) => term)
    .filter((term) => term.length > 1 && !stopWords.has(term));

const trackedProvider = (
  provider: LlmProvider,
  onCall: () => void | Promise<void>,
): LlmProvider => {
  const complete = (async <Output = JsonValue>(
    request: ProviderRequest<Output>,
  ) => {
    await onCall();
    return provider.complete(request);
  }) as LlmProvider['complete'];

  return {
    metadata: provider.metadata,
    capabilities: provider.capabilities,
    complete,
    stream: provider.stream.bind(provider) as LlmProvider['stream'],
    embedding: provider.embedding.bind(provider),
    rerank: provider.rerank.bind(provider),
    models: provider.models.bind(provider),
    validateModel: provider.validateModel.bind(provider),
  };
};

const cacheKey = (
  operation: PlanningModelOperation,
  system: string,
  input: string,
): string =>
  `${operation}:${createHash('sha256').update(system).update('\0').update(input).digest('hex')}`;

const compareText = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;

const rounded = (value: number): number => Number(value.toFixed(12));
