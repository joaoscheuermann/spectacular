import type { Case, ExecutionRecord } from '../schemas/index.js';
import { SKILLS } from '../catalog/index.js';
import { CONDITIONS } from '../conditions/index.js';
import { BASE_TOOL_NAMES, TOOL_NAMES } from '../config/index.js';
import type { JsonValue } from '../core/json.js';
import type { StoredEvent } from '../runtime/index.js';

export interface EvidenceEvaluation {
  readonly success: boolean;
  readonly terminalExact: boolean;
  readonly skillsExact: boolean;
  readonly toolsExact: boolean;
  readonly observationExact: boolean;
  readonly revisionExact: boolean;
  readonly deliveryExact: boolean;
  readonly worldExact: boolean;
  readonly selectedSkills: readonly string[];
  readonly calledTools: readonly string[];
  readonly retrievals?: readonly EvidenceRetrieval[];
  readonly actualMenu?: readonly string[];
  readonly expectedMenu?: readonly string[];
  readonly actualEvidence: readonly HashedObservation[];
  readonly expectedEvidence: readonly HashedObservation[];
}

export interface EvidenceRetrieval {
  readonly goalId: string;
  readonly ranked: readonly string[];
  readonly k: number;
}

export interface HashedObservation {
  readonly tool: string;
  readonly output: string;
}

interface CorrelatedObservation extends HashedObservation {
  readonly sequence: number;
  readonly callId: string;
  readonly nodeId?: string;
  readonly revision?: number;
}

interface ToolStart {
  readonly sequence: number;
  readonly name: string;
  readonly nodeId?: string;
  readonly revision?: number;
}

export interface DerivedScoreEvidence {
  readonly assertions: readonly {
    readonly id: string;
    readonly passed: boolean;
    readonly required: boolean;
  }[];
  readonly retrievals?: readonly EvidenceRetrieval[];
  readonly selectedSkills: readonly string[];
  readonly actualMenu?: readonly string[];
  readonly expectedMenu?: readonly string[];
  readonly observations: {
    readonly actual: readonly HashedObservation[];
    readonly expected: readonly HashedObservation[];
  };
  readonly failureCode?: string;
}

const object = (
  value: JsonValue,
): Readonly<Record<string, JsonValue>> | undefined =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Readonly<Record<string, JsonValue>>)
    : undefined;

const strings = (value: JsonValue | undefined): readonly string[] =>
  Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === 'string')
    : [];

const unique = (values: readonly string[]): readonly string[] => [
  ...new Set(values),
];

const stringField = (
  payload: Readonly<Record<string, JsonValue>>,
  name: string,
): string | undefined =>
  typeof payload[name] === 'string' ? payload[name] : undefined;

const numberField = (
  payload: Readonly<Record<string, JsonValue>>,
  name: string,
): number | undefined =>
  typeof payload[name] === 'number' && Number.isSafeInteger(payload[name])
    ? payload[name]
    : undefined;

const traceEvidence = (events: readonly StoredEvent[]) => {
  const payloads = events.flatMap((event) => {
    const payload = object(event.payload);
    return payload === undefined ? [] : [{ sequence: event.sequence, payload }];
  });
  const selectedSkills = unique(
    payloads.flatMap(({ payload }) => {
      const type = payload['type'];
      return type === 'baseline.menu' || type === 'bundle.selected'
        ? strings(payload['skillNames'])
        : [];
    }),
  );
  const calledTools = unique(
    payloads.flatMap(({ payload }) => {
      if (payload['type'] !== 'tool.call.started') return [];
      const name = payload['name'];
      return typeof name === 'string' ? [name] : [];
    }),
  );
  const duplicateStarts = new Set<string>();
  const starts = payloads.reduce((values, { sequence, payload }) => {
    if (payload['type'] !== 'tool.call.started') return values;
    const callId = stringField(payload, 'callId');
    const name = stringField(payload, 'name');
    if (callId === undefined || name === undefined) return values;
    if (values.has(callId)) {
      duplicateStarts.add(callId);
      return values;
    }
    const nodeId = stringField(payload, 'nodeId');
    const revision = numberField(payload, 'revision');
    values.set(callId, {
      sequence,
      name,
      ...(nodeId === undefined ? {} : { nodeId }),
      ...(revision === undefined ? {} : { revision }),
    });
    return values;
  }, new Map<string, ToolStart>());
  const correlated = payloads.flatMap(
    ({ sequence, payload }): readonly CorrelatedObservation[] => {
      if (payload['type'] !== 'tool.call.finished') return [];
      const callId = stringField(payload, 'callId');
      const name = stringField(payload, 'name');
      const output = stringField(payload, 'evidenceHash');
      if (
        callId === undefined ||
        name === undefined ||
        output === undefined ||
        duplicateStarts.has(callId)
      )
        return [];
      const start = starts.get(callId);
      const nodeId = stringField(payload, 'nodeId');
      const revision = numberField(payload, 'revision');
      if (
        start === undefined ||
        start.sequence >= sequence ||
        start.name !== name ||
        start.nodeId !== nodeId ||
        start.revision !== revision
      ) {
        return [];
      }
      return [
        {
          tool: name,
          output,
          sequence,
          callId,
          ...(nodeId === undefined ? {} : { nodeId }),
          ...(revision === undefined ? {} : { revision }),
        },
      ];
    },
  );
  const actualEvidence = correlated.map(({ tool, output }) => ({
    tool,
    output,
  }));
  const retrievals = payloads.flatMap(({ payload }) => {
    if (payload['type'] !== 'retrieval.result') return [];
    const goalId =
      stringField(payload, 'goalId') ?? stringField(payload, 'nodeId');
    if (goalId === undefined) return [];
    const ranked = strings(payload['skillNames']);
    return [{ goalId, ranked, k: numberField(payload, 'k') ?? ranked.length }];
  });
  const actualMenu = unique(
    payloads.flatMap(({ payload }) =>
      payload['type'] === 'baseline.menu' || payload['type'] === 'menu.composed'
        ? strings(payload['toolNames'])
        : [],
    ),
  );
  const menuObserved = payloads.some(
    ({ payload }) =>
      payload['type'] === 'baseline.menu' ||
      payload['type'] === 'menu.composed',
  );
  const revisions = payloads.flatMap(({ sequence, payload }) => {
    if (
      payload['type'] !== 'baseline.revision' &&
      payload['type'] !== 'graph.revised'
    )
      return [];
    const nodeId =
      stringField(payload, 'goalId') ?? stringField(payload, 'nodeId');
    const revision = numberField(payload, 'revision');
    return nodeId === undefined || revision === undefined
      ? []
      : [{ sequence, nodeId, revision }];
  });
  const revisionSupported = revisions.some((revision) =>
    correlated.some(
      (observation) =>
        observation.sequence < revision.sequence &&
        observation.nodeId === revision.nodeId &&
        observation.revision === revision.revision - 1,
    ),
  );
  return {
    selectedSkills,
    calledTools,
    actualEvidence,
    retrievals,
    actualMenu,
    menuObserved,
    revised: revisions.length > 0,
    revisionSupported,
  };
};

const containsAll = (
  values: readonly string[],
  required: readonly string[],
): boolean => required.every((value) => values.includes(value));

const containsNone = (
  values: readonly string[],
  forbidden: readonly string[],
): boolean => forbidden.every((value) => !values.includes(value));

const deliveryMarkers = (benchmarkCase: Case): readonly string[] => {
  const delivery = object(benchmarkCase.gold.expectedDelivery);
  return strings(delivery?.['contains']);
};

const expectedEvidence = (benchmarkCase: Case): readonly HashedObservation[] =>
  benchmarkCase.gold.expectedState.toolEvidence.map(
    ({ name, evidenceHash }) => ({ tool: name, output: evidenceHash }),
  );

const expectedMenu = (
  record: ExecutionRecord,
  selectedSkills: readonly string[],
  benchmarkCase: Case,
): readonly string[] | undefined => {
  const condition = CONDITIONS.find(({ id }) => id === record.run.conditionId);
  if (condition === undefined) return undefined;
  if (condition.factors.menu === 'global') return TOOL_NAMES;
  if (condition.factors.menu === 'oracle')
    return benchmarkCase.gold.requiredTools;
  const catalog = new Map(SKILLS.map((skill) => [skill.id, skill]));
  const declared = selectedSkills.flatMap(
    (id) => catalog.get(id)?.allowedTools ?? [],
  );
  const names =
    condition.factors.menu === 'base-plus-bundle' && condition.factors.baseTools
      ? [...BASE_TOOL_NAMES, ...declared]
      : declared;
  return unique(names);
};

/** Deterministically evaluates end-to-end evidence from the record and verified trace. */
export const evaluateEvidence = (
  benchmarkCase: Case,
  record: ExecutionRecord,
  events: readonly StoredEvent[],
): EvidenceEvaluation => {
  if (record.run.caseId !== benchmarkCase.id)
    throw new TypeError('execution record does not belong to case');
  if (
    events.some(
      (event) =>
        event.runId !== record.run.id || event.attempt !== record.attempt,
    )
  ) {
    throw new TypeError('stored events do not belong to the execution attempt');
  }
  const evidence = traceEvidence(events);
  const terminalExact =
    record.status === 'succeeded' && record.outcome !== null;
  const skillsExact =
    containsAll(evidence.selectedSkills, benchmarkCase.gold.requiredSkills) &&
    containsNone(evidence.selectedSkills, benchmarkCase.gold.forbiddenSkills);
  const toolsExact =
    containsAll(evidence.calledTools, benchmarkCase.gold.requiredTools) &&
    containsNone(evidence.calledTools, benchmarkCase.gold.forbiddenTools);
  const frozenEvidence = expectedEvidence(benchmarkCase);
  const frozenMenu = expectedMenu(
    record,
    evidence.selectedSkills,
    benchmarkCase,
  );
  const observationExact =
    evidence.actualEvidence.length === frozenEvidence.length &&
    frozenEvidence.every((expected, index) => {
      const actual = evidence.actualEvidence[index];
      return (
        actual?.tool === expected.tool && actual.output === expected.output
      );
    });
  const revisionExact =
    evidence.revised === benchmarkCase.gold.requiresRevision &&
    (!benchmarkCase.gold.requiresRevision || evidence.revisionSupported);
  const outcome = object(record.outcome ?? null);
  const delivery = object(outcome?.['delivery'] ?? null);
  const partMarkdown = Array.isArray(delivery?.['parts'])
    ? delivery['parts'].flatMap((part) => {
        const value = object(part);
        return typeof value?.['markdown'] === 'string'
          ? [value['markdown']]
          : [];
      })
    : [];
  const goalOutputs = Array.isArray(outcome?.['goals'])
    ? outcome['goals'].flatMap((goal) => {
        const value = object(goal);
        return typeof value?.['output'] === 'string' ? [value['output']] : [];
      })
    : [];
  const deliveryText = [
    typeof delivery?.['markdown'] === 'string' ? delivery['markdown'] : '',
    ...partMarkdown,
    ...goalOutputs,
  ].join('\n');
  const deliveryExact = deliveryMarkers(benchmarkCase).every((marker) =>
    deliveryText.includes(marker),
  );
  const worldExact =
    record.worldHash === benchmarkCase.gold.expectedState.worldHash;
  return {
    success:
      terminalExact &&
      toolsExact &&
      observationExact &&
      deliveryExact &&
      worldExact,
    terminalExact,
    skillsExact,
    toolsExact,
    observationExact,
    revisionExact,
    deliveryExact,
    worldExact,
    selectedSkills: evidence.selectedSkills,
    calledTools: evidence.calledTools,
    ...(evidence.retrievals.length === 0
      ? {}
      : { retrievals: evidence.retrievals }),
    ...(evidence.menuObserved ? { actualMenu: evidence.actualMenu } : {}),
    ...(frozenMenu === undefined ? {} : { expectedMenu: frozenMenu }),
    actualEvidence: evidence.actualEvidence,
    expectedEvidence: frozenEvidence,
  };
};

/** Converts only trace-derived deterministic evidence into the scorer input shape. */
export const toScoreEvidence = (
  evaluation: EvidenceEvaluation,
  record: ExecutionRecord,
): DerivedScoreEvidence => {
  const assertions = [
    { id: 'terminal', passed: evaluation.terminalExact, required: true },
    { id: 'skills', passed: evaluation.skillsExact, required: false },
    { id: 'tools', passed: evaluation.toolsExact, required: true },
    { id: 'observations', passed: evaluation.observationExact, required: true },
    { id: 'revision', passed: evaluation.revisionExact, required: false },
    { id: 'delivery', passed: evaluation.deliveryExact, required: true },
    { id: 'world', passed: evaluation.worldExact, required: true },
  ] as const;
  const outcome = object(record.outcome ?? null);
  const outcomeFailure =
    typeof outcome?.['failureCode'] === 'string'
      ? outcome['failureCode']
      : undefined;
  const failureCode = evaluation.success
    ? undefined
    : (record.infrastructureFailure?.code ?? outcomeFailure);
  return {
    assertions,
    ...(evaluation.retrievals === undefined
      ? {}
      : { retrievals: evaluation.retrievals }),
    selectedSkills: evaluation.selectedSkills,
    ...(evaluation.actualMenu === undefined
      ? {}
      : { actualMenu: evaluation.actualMenu }),
    ...(evaluation.expectedMenu === undefined
      ? {}
      : { expectedMenu: evaluation.expectedMenu }),
    observations: {
      actual: evaluation.actualEvidence,
      expected: evaluation.expectedEvidence,
    },
    ...(failureCode === undefined ? {} : { failureCode }),
  };
};
