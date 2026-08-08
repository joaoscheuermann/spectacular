import type { Case } from '../schemas/index.js';
import { CaseV1 } from '../schemas/index.js';
import {
  SKILLS,
  type CatalogDomain,
  type MicroSkill,
} from '../catalog/index.js';
import { TOOL_NAMES, type ToolName } from '../config/index.js';
import { artifactHash } from '../core/hash.js';
import type { JsonValue } from '../core/json.js';
import { executeTool } from '../runtime/tools.js';
import { createWorld, worldHash } from '../runtime/world.js';

const CLASSES = ['A', 'B', 'C', 'D', 'E', 'F'] as const;
const MUTATING_TOOLS: readonly ToolName[] = [
  'write',
  'artifact_publish',
  'message_send',
];

const subjectFor = (skill: MicroSkill, number: number): string => {
  void number;
  const subject = {
    'documents-finance': 'invoice-001 and ledger-001 in world-v1',
    software: 'the reconcile symbol and api component in world-v1',
    artifacts: 'the report template in world-v1',
    communication: 'alice and msg-000 in world-v1',
  } satisfies Record<CatalogDomain, string>;
  return subject[skill.domain];
};

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
    .map((relation) =>
      SKILLS.find((candidate) => candidate.id === relation.skillId),
    )
    .filter((candidate): candidate is MicroSkill => candidate !== undefined);
  const blocked = new Set(
    skill.relations
      .filter(
        (relation) =>
          relation.kind === 'conflict' || relation.kind === 'distractor',
      )
      .map((relation) => relation.skillId),
  );
  const domain = SKILLS.filter(
    (candidate) =>
      candidate.domain === skill.domain &&
      candidate.id !== skill.id &&
      !blocked.has(candidate.id),
  );
  const selected: MicroSkill[] = [skill];
  for (const candidate of [...peers, ...domain]) {
    if (!selected.some((entry) => entry.id === candidate.id))
      selected.push(candidate);
    const tools = new Set(selected.flatMap((entry) => entry.allowedTools));
    if (selected.length >= 2 && tools.size >= 2) return selected;
  }
  throw new Error(`domain cannot satisfy class F composition: ${skill.domain}`);
};

const compositionFor = (skill: MicroSkill, index: number) => {
  const compositionClass = CLASSES[index % CLASSES.length];
  const support = supportSkills(skill);
  const declaredTools = [
    ...new Set(support.flatMap((entry) => entry.allowedTools)),
  ];
  const optionalBase =
    Math.floor(index / CLASSES.length) % 2 === 0 ? [] : ['read' as const];
  const values = {
    A: {
      skills: [] as readonly MicroSkill[],
      tools: [] as readonly ToolName[],
      toolRequirement: 'none' as const,
    },
    B: {
      skills: [] as readonly MicroSkill[],
      tools: ['read'] as readonly ToolName[],
      toolRequirement: 'base' as const,
    },
    C: {
      skills: [skill] as readonly MicroSkill[],
      tools: [] as readonly ToolName[],
      toolRequirement: 'none' as const,
    },
    D: {
      skills: [skill] as readonly MicroSkill[],
      tools: [skill.allowedTools[0] as ToolName],
      toolRequirement: 'declared' as const,
    },
    E: {
      skills: support.slice(0, 2),
      tools: optionalBase,
      toolRequirement:
        optionalBase.length === 0 ? ('none' as const) : ('base' as const),
    },
    F: {
      skills: support,
      tools: declaredTools.slice(0, 2),
      toolRequirement: 'declared' as const,
    },
  };
  return { compositionClass, ...values[compositionClass] };
};

const toolInput = (name: ToolName, marker: string): JsonValue =>
  (
    ({
      list: { prefix: 'notes' },
      read: { path: 'notes/request.md' },
      write: { path: `outputs/${marker}.md`, content: `${marker} world-v1` },
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
        value: { title: marker, summary: 'world-v1' },
      },
      render_preview: {
        format: 'markdown',
        value: { title: marker, summary: 'world-v1' },
      },
      artifact_publish: { value: { title: marker, summary: 'world-v1' } },
      channel_list: {},
      recipient_resolve: { query: 'alice' },
      message_send: {
        recipientId: 'person-alice',
        channelId: 'direct',
        body: `${marker} world-v1`,
      },
      message_status: { id: 'msg-000' },
    }) satisfies Readonly<Record<ToolName, JsonValue>>
  )[name];

const describeInput = (value: JsonValue): string => {
  if (Array.isArray(value)) return `[${value.map(describeInput).join(', ')}]`;
  if (typeof value === 'object' && value !== null) {
    const fields = Object.entries(value).map(
      ([key, entry]) => `${key}=${describeInput(entry)}`,
    );
    return fields.length === 0 ? 'no arguments' : fields.join(', ');
  }
  return typeof value === 'string' ? `"${value}"` : String(value);
};

const operationInstruction = (name: ToolName, marker: string): string =>
  `Use ${name} exactly once with ${describeInput(toolInput(name, marker))}.`;

const expectedExecution = (tools: readonly ToolName[], marker: string) => {
  let world = createWorld();
  const toolEvidence = tools.map((name) => {
    const input = toolInput(name, marker);
    const execution = executeTool(world, name, input);
    world = execution.world;
    return {
      name,
      input,
      evidenceHash: artifactHash({ name, input, output: execution.output }),
    };
  });
  return { worldHash: worldHash(world), toolEvidence };
};

const requestFor = (
  skill: MicroSkill,
  index: number,
  composition: ReturnType<typeof compositionFor>,
  marker: string,
): string => {
  const subject = subjectFor(skill, index + 1);
  const secondary = composition.skills[1]?.description;
  const operations = composition.tools
    .map((name) => operationInstruction(name, marker))
    .join(' ');
  const requests = {
    A: `Return the identifier world-v1 from this request. Do not use a skill or tool.`,
    B: `Report the exact fixture identifier from notes/request.md. No specialized skill is needed. ${operations}`,
    C: `Using the supplied procedure only, explain how to achieve this outcome without external evidence: ${skill.description}`,
    D: `${skill.description} Use ${subject}. ${operations}`,
    E: `Coordinate both supplied procedures: ${skill.description} ${secondary ?? ''} ${operations}`,
    F: `Coordinate the supplied procedures for ${subject}: ${skill.description} ${secondary ?? ''} ${operations}`,
  } as const;
  return `${requests[composition.compositionClass]} Preserve fixture identifiers and report every required criterion.`;
};

const materialize = (skill: MicroSkill, index: number): Case => {
  const composition = compositionFor(skill, index);
  const { compositionClass } = composition;
  const ordinal = String(index + 1).padStart(2, '0');
  const deliveryMarker = `pilot-${ordinal}-complete`;
  const requiresExternalEffect = composition.tools.some((tool) =>
    MUTATING_TOOLS.includes(tool),
  );
  const requiresRevision =
    index % 5 === 0 && composition.tools.length > 0 && !requiresExternalEffect;
  const expected = expectedExecution(composition.tools, deliveryMarker);
  const relevantSkills = composition.skills.flatMap((entry) => [
    entry.id,
    ...related(entry, ['equivalent', 'overlap']),
  ]);
  const requiredToolSet = new Set<ToolName>(composition.tools);
  const base = {
    schemaVersion: 1 as const,
    id: `pilot.case.${ordinal}`,
    familyId: `pilot.family.${ordinal}`,
    phase: 'pilot' as const,
    title: `Pilot ${ordinal}: ${skill.title}`,
    domain: skill.domain,
    compositionClass,
    focusGoalRole: 'goal.primary',
    composition: {
      skillCount:
        composition.skills.length === 0
          ? (0 as const)
          : composition.skills.length === 1
            ? (1 as const)
            : ('many' as const),
      toolRequirement: composition.toolRequirement,
      toolCount:
        composition.tools.length === 0
          ? (0 as const)
          : composition.tools.length === 1
            ? (1 as const)
            : ('many' as const),
      requiresRevision,
      requiresExternalEffect,
    },
    adaptive: requiresRevision || index % 4 === 0,
    request: `${requestFor(skill, index, composition, deliveryMarker)}${requiresRevision ? ' Treat the first tool observation as a structural invalidation: request one localized revision after inspecting it, then complete the revised goal.' : ''} The final delivery must include ${deliveryMarker} and world-v1.`,
    fixtureIds: ['world-v1'],
    tags: ['pilot', `domain.${skill.domain}`, `class.${compositionClass}`],
    gold: {
      criteria: [
        {
          id: 'uses-required-tools',
          description:
            'Uses only the required deterministic evidence operations.',
        },
        {
          id: 'reports-outcome',
          description:
            'Reports the requested outcome with fixture identifiers.',
        },
      ],
      requiredSkills: composition.skills.map((entry) => entry.id),
      relevantSkills: [...new Set(relevantSkills)],
      forbiddenSkills:
        composition.skills.length === 0
          ? SKILLS.map((entry) => entry.id)
          : related(skill, ['distractor', 'conflict']),
      requiredTools: [...composition.tools],
      forbiddenTools: TOOL_NAMES.filter((tool) => !requiredToolSet.has(tool)),
      expectedState: {
        fixtureId: 'world-v1',
        worldHash: expected.worldHash,
        toolEvidence: expected.toolEvidence,
        requiredEffects: composition.tools.filter((tool) =>
          MUTATING_TOOLS.includes(tool),
        ),
      },
      expectedDelivery: { contains: [deliveryMarker, 'world-v1'] },
      requiresRevision,
    },
  };

  return CaseV1.parse({ ...base, contentHash: artifactHash(base) });
};

/** Sixty pilot families balanced 15/domain and 10/composition class. */
export const PILOT_CASES: readonly Case[] = SKILLS.map(materialize);

export * from './case-validation.js';
