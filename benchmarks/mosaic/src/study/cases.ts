import {
  SKILLS,
  type CatalogDomain,
  type MicroSkill,
} from '../catalog/index.js';
import { type ToolName } from '../config/index.js';
import type { JsonValue } from '../core/json.js';
import { compileCaseDrafts } from './case-compiler.js';

const CLASSES = ['A', 'B', 'C', 'D', 'E', 'F'] as const;

const related = (
  skill: MicroSkill,
  kinds: readonly string[],
): readonly string[] =>
  skill.relations
    .filter((relation) => kinds.includes(relation.kind))
    .map((relation) => relation.skillId);

const supportSkills = (skill: MicroSkill): readonly MicroSkill[] => {
  const peers = skill.relations
    .filter(
      (relation) =>
        relation.kind === 'equivalent' || relation.kind === 'overlap',
    )
    .flatMap((relation) =>
      SKILLS.filter((candidate) => candidate.id === relation.skillId),
    );
  const blocked = new Set(related(skill, ['conflict', 'distractor']));
  const domain = SKILLS.filter(
    (candidate) =>
      candidate.domain === skill.domain &&
      candidate.id !== skill.id &&
      !blocked.has(candidate.id),
  );
  const selected: MicroSkill[] = [skill];
  for (const candidate of [...peers, ...domain]) {
    if (!selected.some(({ id }) => id === candidate.id))
      selected.push(candidate);
    if (
      selected.length >= 2 &&
      new Set(selected.flatMap(({ allowedTools }) => allowedTools)).size >= 2
    ) {
      return selected;
    }
  }
  throw new Error(`domain cannot satisfy class F composition: ${skill.domain}`);
};

const compositionFor = (skill: MicroSkill, index: number) => {
  const compositionClass = CLASSES[index % CLASSES.length];
  const support = supportSkills(skill);
  const declared = [
    ...new Set(support.flatMap(({ allowedTools }) => allowedTools)),
  ];
  const optionalBase =
    Math.floor(index / CLASSES.length) % 2 === 0 ? [] : ['read' as const];
  const values = {
    A: { skills: [], tools: [] },
    B: { skills: [], tools: ['read' as const] },
    C: { skills: [skill], tools: [] },
    D: { skills: [skill], tools: [skill.allowedTools[0] as ToolName] },
    E: { skills: support.slice(0, 2), tools: optionalBase },
    F: { skills: support, tools: declared.slice(0, 2) },
  } satisfies Readonly<
    Record<
      (typeof CLASSES)[number],
      {
        readonly skills: readonly MicroSkill[];
        readonly tools: readonly ToolName[];
      }
    >
  >;
  return { compositionClass, ...values[compositionClass] };
};

const toolInput = (name: ToolName, ordinal: string): JsonValue =>
  (
    ({
      list: { prefix: 'notes' },
      read: { path: 'notes/request.md' },
      write: {
        path: `outputs/pilot-${ordinal}.md`,
        content: 'Invoice invoice-001 differs from ledger-001 by USD 5.',
      },
      search: { query: 'world-v1' },
      calculate: { operation: 'subtract', values: [125, 120] },
      json_query: { path: 'data/settings.json', query: 'currency' },
      document_search: { query: 'ledger-001' },
      document_fetch: { id: 'invoice-001' },
      record_lookup: { id: 'ledger-001' },
      timeseries_query: { series: 'cashflow' },
      currency_convert: { from: 'USD', to: 'BRL', amount: 125 },
      portfolio_snapshot: {},
      symbol_lookup: { name: 'reconcile' },
      dependency_query: { component: 'api', direction: 'dependencies' },
      test_run: { target: 'api' },
      build_check: { target: 'api' },
      template_get: { name: 'report' },
      schema_validate: {
        name: 'report',
        value: { title: 'Reconciliation', summary: 'USD 5 variance' },
      },
      render_preview: {
        format: 'markdown',
        value: { title: 'Reconciliation', summary: 'USD 5 variance' },
      },
      artifact_publish: {
        value: { title: 'Reconciliation', summary: 'USD 5 variance' },
      },
      channel_list: {},
      recipient_resolve: { query: 'alice' },
      message_send: {
        recipientId: 'person-alice',
        channelId: 'direct',
        body: 'Invoice invoice-001 has a USD 5 reconciliation variance.',
      },
      message_status: { id: 'msg-000' },
    }) satisfies Readonly<Record<ToolName, JsonValue>>
  )[name];

const describe = (value: JsonValue): string => {
  if (Array.isArray(value)) return `[${value.map(describe).join(', ')}]`;
  if (typeof value === 'object' && value !== null) {
    const fields = Object.entries(value).map(
      ([key, entry]) => `${key}=${describe(entry)}`,
    );
    return fields.length === 0 ? 'no arguments' : fields.join(', ');
  }
  return typeof value === 'string' ? `"${value}"` : String(value);
};

const TASKS = {
  'documents-finance': {
    statement:
      'Reconcile invoice invoice-001 at USD 125 with ledger record ledger-001 at USD 120.',
    answer: {
      invoiceId: 'invoice-001',
      ledgerId: 'ledger-001',
      differenceUsd: 5,
    },
    instruction:
      'Set answer to invoiceId, ledgerId, and the numeric differenceUsd.',
  },
  software: {
    statement:
      'Summarize the stated API relationship: component api depends on core and exports reconcile.',
    answer: {
      component: 'api',
      dependency: 'core',
      exportedSymbol: 'reconcile',
    },
    instruction: 'Set answer to component, dependency, and exportedSymbol.',
  },
  artifacts: {
    statement:
      'Define the required report artifact whose template needs title and summary fields.',
    answer: { template: 'report', requiredFields: ['title', 'summary'] },
    instruction: 'Set answer to template and the ordered requiredFields array.',
  },
  communication: {
    statement:
      'Prepare the exact direct destination for Alice, whose recipient identifier is person-alice.',
    answer: { recipientId: 'person-alice', channelId: 'direct' },
    instruction: 'Set answer to recipientId and channelId.',
  },
} satisfies Readonly<
  Record<
    CatalogDomain,
    {
      readonly statement: string;
      readonly answer: JsonValue;
      readonly instruction: string;
    }
  >
>;

const task = (domain: CatalogDomain) => TASKS[domain];

const draftFor = (skill: MicroSkill, index: number) => {
  const composition = compositionFor(skill, index);
  const ordinal = String(index + 1).padStart(2, '0');
  const tools = composition.tools.map((name, toolIndex) => ({
    id: `tool.${toolIndex + 1}.${name}`,
    name,
    input: toolInput(name, ordinal),
  }));
  const semantic = task(skill.domain);
  const operations = tools
    .map(
      (tool) =>
        `Use ${tool.name} exactly once with ${describe(tool.input)} and preserve its exact JSON output under observations["${tool.id}"].`,
    )
    .join(' ');
  const secondary = composition.skills[1]?.description;
  const procedures =
    composition.skills.length === 0
      ? 'No specialized procedure is required.'
      : `Apply the supplied procedures: ${skill.description}${secondary === undefined ? '' : ` ${secondary}`}`;
  const requiresRevision =
    index % 5 === 0 &&
    tools.length > 0 &&
    !tools.some(({ name }) =>
      ['write', 'artifact_publish', 'message_send'].includes(name),
    );
  const request = `${semantic.statement} ${procedures} ${operations} Return exactly one JSON object with fixtureId, answer, and observations. ${semantic.instruction} Set fixtureId from the isolated fixture and include every requested observation; use an empty observations object when no operation is requested.${requiresRevision ? ' Treat the first tool observation as a structural invalidation: request one localized revision after inspecting it, then complete the revised goal.' : ''}`;
  const relevantSkills = composition.skills.flatMap((entry) => [
    entry.id,
    ...related(entry, ['equivalent', 'overlap']),
  ]);
  return {
    schemaVersion: 1,
    id: `pilot.case.${ordinal}`,
    familyId: `pilot.family.${ordinal}`,
    phase: 'pilot',
    title: `Pilot ${ordinal}: ${skill.title}`,
    domain: skill.domain,
    compositionClass: composition.compositionClass,
    focusGoalRole: 'goal.primary',
    adaptive: requiresRevision || index % 4 === 0,
    request,
    fixtureIds: ['world-v1'],
    tags: [
      'pilot',
      `domain.${skill.domain}`,
      `class.${composition.compositionClass}`,
    ],
    criteria: [
      {
        id: 'task-evidence',
        description:
          'The substantive requested answer and operations are correct.',
        evidenceRefs: ['delivery.answer', ...tools.map(({ id }) => id)],
      },
      {
        id: 'reports-outcome',
        description:
          'The canonical delivery reports the fixture and observations.',
        evidenceRefs: [
          'delivery.fixture',
          ...tools.map(({ id }) => `delivery.${id}`),
        ],
      },
    ],
    requiredSkills: composition.skills.map(({ id }) => id),
    relevantSkills: [...new Set(relevantSkills)],
    forbiddenSkills: related(skill, ['distractor', 'conflict']),
    tools,
    expectedAnswer: semantic.answer,
    requiresRevision,
  };
};

const compiled = compileCaseDrafts({
  schemaVersion: 1,
  cases: SKILLS.map(draftFor),
});
if (!compiled.valid || compiled.cases.length !== 60) {
  throw new Error(
    `pilot case compilation failed: ${JSON.stringify(compiled.issues)}`,
  );
}

/** Sixty pilot families balanced 15/domain and 10/composition class. */
export const PILOT_CASES = compiled.cases;

export * from './case-validation.js';
