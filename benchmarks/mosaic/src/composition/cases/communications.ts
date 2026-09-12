import type { PlanningCase } from '../planning-schema.js';
import {
  buildPlanningCase,
  catalogBehavior,
  criterion,
  dependency,
  type DomainFixture,
  phase,
  requestBehavior,
} from './shared.js';

const fixture: DomainFixture = {
  domain: 'communications',
  behaviors: [
    catalogBehavior(
      'communication.adapt-audience',
      'Adjust detail and terminology to the named audience.',
    ),
    catalogBehavior(
      'communication.resolve-recipient',
      'Resolve a human-readable recipient before sending a message.',
    ),
    catalogBehavior(
      'communication.assess-risk',
      'State impact, uncertainty, and the next decision without overstating risk.',
    ),
    catalogBehavior(
      'communication.write-concisely',
      'Lead with the decision and remove details that do not change action.',
    ),
    catalogBehavior(
      'communication.route-incident',
      'Choose the incident channel and recipients from severity and ownership.',
    ),
    catalogBehavior(
      'communication.verify-delivery',
      'Check the sent message status instead of assuming delivery.',
    ),
    catalogBehavior(
      'communication.add-promotion',
      'Add persuasive promotional claims and a marketing call to action.',
    ),
  ],
  skills: [
    {
      id: 'skill.audience-adaptation',
      name: 'audience-adaptation',
      description: 'Adapts technical content to a named audience.',
      body: 'Preserve the underlying facts while adjusting terminology, detail, and assumed context for the named audience. Do not simplify away a decision-relevant risk.',
      behaviorIds: ['communication.adapt-audience'],
      toolIds: [],
    },
    {
      id: 'skill.recipient-resolution',
      name: 'recipient-resolution',
      description: 'Resolves and contacts a named recipient.',
      body: 'Resolve the human-readable recipient to one unambiguous identifier before sending. Stop rather than guessing when multiple candidates remain.',
      behaviorIds: ['communication.resolve-recipient'],
      toolIds: ['recipient-resolve', 'message-send'],
    },
    {
      id: 'skill.risk-communication',
      name: 'risk-communication',
      description: 'Communicates operational risk without distortion.',
      body: 'State observed impact, uncertainty, and the next decision separately. Avoid certainty or severity claims not supported by the supplied facts.',
      behaviorIds: ['communication.assess-risk'],
      toolIds: [],
    },
    {
      id: 'skill.concise-writing',
      name: 'concise-writing',
      description: 'Makes action-oriented messages compact.',
      body: 'Lead with the decision or requested action, retain only facts that change that action, and move optional background out of the opening.',
      behaviorIds: ['communication.write-concisely'],
      toolIds: [],
    },
    {
      id: 'skill.incident-routing',
      name: 'incident-routing',
      description: 'Routes incidents by severity and ownership.',
      body: 'Inspect available channels, use the stated severity and owner to select the route, and resolve every named participant before delivery.',
      behaviorIds: ['communication.route-incident'],
      toolIds: ['channel-list', 'recipient-resolve'],
    },
    {
      id: 'skill.delivery-verification',
      name: 'delivery-verification',
      description: 'Sends a message and verifies its delivery state.',
      body: 'Send the approved message once, retain its identifier, and query status before reporting delivery as successful.',
      behaviorIds: ['communication.verify-delivery'],
      toolIds: ['message-send', 'message-status'],
    },
    {
      id: 'skill.marketing-copy',
      name: 'marketing-copy',
      description: 'Adds promotional framing and calls to action.',
      body: 'Emphasize benefits with persuasive language and a promotional call to action. Do not apply this style to operational incidents.',
      behaviorIds: ['communication.add-promotion'],
      toolIds: [],
    },
  ],
};

const direct = buildPlanningCase(fixture, {
  id: 'planning.communications.a',
  title: 'Draft a supplied factual update',
  compositionClass: 'A',
  request:
    'Draft one sentence stating that maintenance starts at 22:00 UTC and is expected to last 20 minutes. Do not send it and do not add promotional language.',
  requestBehaviors: [
    requestBehavior(
      'communication.preserve-maintenance-facts',
      'Preserve the supplied time and duration in one sentence.',
    ),
  ],
  outputs: [
    {
      id: 'output.maintenance-sentence',
      description: 'One factual maintenance update sentence.',
    },
  ],
  roles: [
    {
      id: 'role.draft-maintenance',
      description: 'Draft the supplied maintenance update.',
      outputIds: ['output.maintenance-sentence'],
      behaviorIds: ['communication.preserve-maintenance-facts'],
    },
  ],
  dependencies: [],
  relevantSkillIds: [],
  baseToolIds: [],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.draft-maintenance',
        ['output.maintenance-sentence'],
        ['communication.preserve-maintenance-facts'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['communication.add-promotion'],
  ),
  p1: phase(
    [
      criterion(
        'role.draft-maintenance',
        ['output.maintenance-sentence'],
        ['communication.preserve-maintenance-facts'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['communication.add-promotion'],
  ),
});

const baseTool = buildPlanningCase(fixture, {
  id: 'planning.communications.b',
  title: 'Look up an opaque directory recipient',
  compositionClass: 'B',
  request:
    'Use the base recipient lookup to find the unique directory identifier for “Dana from Finance”. Return the identifier only and do not send a message.',
  requestBehaviors: [
    requestBehavior(
      'communication.lookup-directory',
      'Query the opaque directory for the named person.',
    ),
    requestBehavior(
      'communication.report-recipient-id',
      'Return the uniquely observed directory identifier without sending.',
    ),
  ],
  outputs: [
    {
      id: 'output.directory-result',
      description: 'The directory lookup result.',
    },
    {
      id: 'output.recipient-id',
      description: 'Dana from Finance’s unique directory identifier.',
    },
  ],
  roles: [
    {
      id: 'role.lookup-recipient',
      description: 'Look up the named person.',
      outputIds: ['output.directory-result'],
      behaviorIds: ['communication.lookup-directory'],
    },
    {
      id: 'role.report-recipient',
      description: 'Return the observed identifier.',
      outputIds: ['output.recipient-id'],
      behaviorIds: ['communication.report-recipient-id'],
    },
  ],
  dependencies: [
    dependency(
      'dependency.lookup-before-recipient',
      'role.lookup-recipient',
      'role.report-recipient',
    ),
  ],
  relevantSkillIds: [],
  baseToolIds: ['recipient-resolve'],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.lookup-recipient',
        ['output.directory-result'],
        ['communication.lookup-directory'],
      ),
      criterion(
        'role.report-recipient',
        ['output.recipient-id'],
        ['communication.report-recipient-id'],
      ),
    ],
    ['dependency.lookup-before-recipient'],
    { min: 2, max: 2 },
    ['communication.verify-delivery', 'communication.add-promotion'],
  ),
  p1: phase(
    [
      criterion(
        'role.lookup-recipient',
        ['output.directory-result'],
        ['communication.lookup-directory'],
      ),
      criterion(
        'role.report-recipient',
        ['output.recipient-id'],
        ['communication.report-recipient-id'],
      ),
    ],
    ['dependency.lookup-before-recipient'],
    { min: 2, max: 2 },
    ['communication.verify-delivery', 'communication.add-promotion'],
  ),
});

const cognitive = buildPlanningCase(fixture, {
  id: 'planning.communications.c',
  title: 'Adapt supplied technical facts for executives',
  compositionClass: 'C',
  request:
    'For an executive audience, rewrite these supplied facts: cache hit rate fell from 94% to 81%, latency increased by 35 ms, and mitigation is active. Preserve all numbers and do not send the message.',
  requestBehaviors: [
    requestBehavior(
      'communication.preserve-incident-numbers',
      'Preserve the supplied cache and latency numbers and mitigation state.',
    ),
  ],
  outputs: [
    {
      id: 'output.executive-update',
      description: 'An executive-facing update with all supplied facts.',
    },
  ],
  roles: [
    {
      id: 'role.adapt-executive-update',
      description: 'Adapt the technical facts for executives.',
      outputIds: ['output.executive-update'],
      behaviorIds: [
        'communication.preserve-incident-numbers',
        'communication.adapt-audience',
      ],
    },
  ],
  dependencies: [],
  relevantSkillIds: ['skill.audience-adaptation'],
  baseToolIds: [],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.adapt-executive-update',
        ['output.executive-update'],
        ['communication.preserve-incident-numbers'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['communication.add-promotion'],
  ),
  p1: phase(
    [
      criterion(
        'role.adapt-executive-update',
        ['output.executive-update'],
        [
          'communication.preserve-incident-numbers',
          'communication.adapt-audience',
        ],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['communication.add-promotion'],
  ),
});

const oneSkillTools = buildPlanningCase(fixture, {
  id: 'planning.communications.d',
  title: 'Resolve and notify one named recipient',
  compositionClass: 'D',
  request:
    'Resolve the unique recipient “Alex Chen, database owner” and send exactly one message: “Failover test begins at 19:00 UTC.” Return the send identifier.',
  requestBehaviors: [
    requestBehavior(
      'communication.send-exact-message',
      'Send the supplied message exactly once.',
    ),
    requestBehavior(
      'communication.return-send-id',
      'Return the identifier from the send operation.',
    ),
  ],
  outputs: [
    {
      id: 'output.resolved-recipient',
      description: 'The unique database-owner recipient identifier.',
    },
    {
      id: 'output.send-id',
      description: 'The identifier of the sent message.',
    },
  ],
  roles: [
    {
      id: 'role.resolve-owner',
      description: 'Resolve the named database owner.',
      outputIds: ['output.resolved-recipient'],
      behaviorIds: ['communication.resolve-recipient'],
    },
    {
      id: 'role.notify-owner',
      description: 'Send the exact notification and retain its identifier.',
      outputIds: ['output.send-id'],
      behaviorIds: [
        'communication.send-exact-message',
        'communication.return-send-id',
      ],
    },
  ],
  dependencies: [
    dependency(
      'dependency.recipient-before-send',
      'role.resolve-owner',
      'role.notify-owner',
    ),
  ],
  relevantSkillIds: ['skill.recipient-resolution'],
  baseToolIds: [],
  declaredToolIds: ['recipient-resolve', 'message-send'],
  p0: phase(
    [
      criterion('role.resolve-owner', ['output.resolved-recipient'], []),
      criterion(
        'role.notify-owner',
        ['output.send-id'],
        ['communication.send-exact-message', 'communication.return-send-id'],
      ),
    ],
    ['dependency.recipient-before-send'],
    { min: 2, max: 3 },
    ['communication.verify-delivery'],
  ),
  p1: phase(
    [
      criterion(
        'role.resolve-owner',
        ['output.resolved-recipient'],
        ['communication.resolve-recipient'],
      ),
      criterion(
        'role.notify-owner',
        ['output.send-id'],
        ['communication.send-exact-message', 'communication.return-send-id'],
      ),
    ],
    ['dependency.recipient-before-send'],
    { min: 2, max: 3 },
    ['communication.verify-delivery'],
  ),
});

const manySkillsSimpleMenu = buildPlanningCase(fixture, {
  id: 'planning.communications.e',
  title: 'Prepare a concise risk update',
  compositionClass: 'E',
  request:
    'Prepare, but do not send, a concise operations update from these facts: one region is degraded, customer impact is uncertain, rollback is ready, and the decision is whether to roll back now.',
  requestBehaviors: [
    requestBehavior(
      'communication.include-risk-facts',
      'Include degradation, uncertainty, rollback readiness, and the decision.',
    ),
  ],
  outputs: [
    {
      id: 'output.risk-update',
      description: 'A concise, action-oriented operational risk update.',
    },
  ],
  roles: [
    {
      id: 'role.prepare-risk-update',
      description: 'Assess and write the operational update.',
      outputIds: ['output.risk-update'],
      behaviorIds: [
        'communication.include-risk-facts',
        'communication.assess-risk',
        'communication.write-concisely',
      ],
    },
  ],
  dependencies: [],
  relevantSkillIds: ['skill.risk-communication', 'skill.concise-writing'],
  baseToolIds: [],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.prepare-risk-update',
        ['output.risk-update'],
        ['communication.include-risk-facts'],
      ),
    ],
    [],
    { min: 1, max: 2 },
    ['communication.add-promotion'],
  ),
  p1: phase(
    [
      criterion(
        'role.prepare-risk-update',
        ['output.risk-update'],
        [
          'communication.include-risk-facts',
          'communication.assess-risk',
          'communication.write-concisely',
        ],
      ),
    ],
    [],
    { min: 1, max: 2 },
    ['communication.add-promotion'],
  ),
});

const manySkillsManyTools = buildPlanningCase(fixture, {
  id: 'planning.communications.f',
  title: 'Route, send, and verify an incident notice',
  compositionClass: 'F',
  request:
    'For severity-1 database incident db-77 owned by the database team, choose the correct incident channel, resolve the on-call owner, send the supplied notice “db-77 mitigation in progress”, and verify delivery.',
  requestBehaviors: [
    requestBehavior(
      'communication.use-incident-facts',
      'Use severity, owner, incident ID, and supplied notice exactly as given.',
    ),
    requestBehavior(
      'communication.report-delivery',
      'Return the channel, recipient, message identifier, and observed status.',
    ),
  ],
  outputs: [
    {
      id: 'output.incident-route',
      description: 'The selected channel and resolved on-call owner.',
    },
    {
      id: 'output.incident-message',
      description: 'The sent message identifier.',
    },
    {
      id: 'output.delivery-status',
      description: 'The observed delivery status and routing summary.',
    },
  ],
  roles: [
    {
      id: 'role.route-incident',
      description: 'Select the channel and resolve the owner.',
      outputIds: ['output.incident-route'],
      behaviorIds: [
        'communication.use-incident-facts',
        'communication.route-incident',
        'communication.resolve-recipient',
      ],
    },
    {
      id: 'role.send-incident',
      description: 'Send the supplied incident notice once.',
      outputIds: ['output.incident-message'],
      behaviorIds: ['communication.use-incident-facts'],
    },
    {
      id: 'role.verify-incident-delivery',
      description: 'Verify and report delivery.',
      outputIds: ['output.delivery-status'],
      behaviorIds: [
        'communication.report-delivery',
        'communication.verify-delivery',
      ],
    },
  ],
  dependencies: [
    dependency(
      'dependency.route-before-send',
      'role.route-incident',
      'role.send-incident',
    ),
    dependency(
      'dependency.send-before-status',
      'role.send-incident',
      'role.verify-incident-delivery',
    ),
  ],
  relevantSkillIds: [
    'skill.incident-routing',
    'skill.recipient-resolution',
    'skill.delivery-verification',
  ],
  baseToolIds: [],
  declaredToolIds: [
    'channel-list',
    'recipient-resolve',
    'message-send',
    'message-status',
  ],
  p0: phase(
    [
      criterion(
        'role.route-incident',
        ['output.incident-route'],
        ['communication.use-incident-facts'],
      ),
      criterion(
        'role.send-incident',
        ['output.incident-message'],
        ['communication.use-incident-facts'],
      ),
      criterion(
        'role.verify-incident-delivery',
        ['output.delivery-status'],
        ['communication.report-delivery'],
      ),
    ],
    ['dependency.route-before-send', 'dependency.send-before-status'],
    { min: 3, max: 4 },
    ['communication.add-promotion'],
  ),
  p1: phase(
    [
      criterion(
        'role.route-incident',
        ['output.incident-route'],
        [
          'communication.use-incident-facts',
          'communication.route-incident',
          'communication.resolve-recipient',
        ],
      ),
      criterion(
        'role.send-incident',
        ['output.incident-message'],
        ['communication.use-incident-facts'],
      ),
      criterion(
        'role.verify-incident-delivery',
        ['output.delivery-status'],
        ['communication.report-delivery', 'communication.verify-delivery'],
      ),
    ],
    ['dependency.route-before-send', 'dependency.send-before-status'],
    { min: 3, max: 4 },
    ['communication.add-promotion'],
  ),
});

export const communicationCases: readonly PlanningCase[] = [
  direct,
  baseTool,
  cognitive,
  oneSkillTools,
  manySkillsSimpleMenu,
  manySkillsManyTools,
];
