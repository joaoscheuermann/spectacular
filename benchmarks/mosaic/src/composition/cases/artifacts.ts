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
  domain: 'artifacts',
  behaviors: [
    catalogBehavior(
      'artifact.organize-hierarchy',
      'Organize content around a clear heading and supporting sections.',
    ),
    catalogBehavior(
      'artifact.satisfy-template',
      'Populate every required template field without adding unknown fields.',
    ),
    catalogBehavior(
      'artifact.check-accessibility',
      'Check reading order, labels, contrast, and non-visual meaning.',
    ),
    catalogBehavior(
      'artifact.prioritize-information',
      'Give the primary decision more visual weight than supporting details.',
    ),
    catalogBehavior(
      'artifact.validate-schema',
      'Validate the complete artifact against its declared schema.',
    ),
    catalogBehavior(
      'artifact.publish-release',
      'Publish only the validated artifact and retain its reference.',
    ),
    catalogBehavior(
      'artifact.add-animation',
      'Add decorative motion and transition effects.',
    ),
  ],
  skills: [
    {
      id: 'skill.editorial-structure',
      name: 'editorial-structure',
      description: 'Turns supplied material into a coherent hierarchy.',
      body: 'Select one primary message, group supporting facts beneath descriptive headings, and keep the hierarchy proportional to the requested artifact.',
      behaviorIds: ['artifact.organize-hierarchy'],
      toolIds: [],
    },
    {
      id: 'skill.template-compliance',
      name: 'template-compliance',
      description: 'Produces artifacts that satisfy a fixed template.',
      body: 'Inspect the template contract, populate every required field, omit unknown fields, and preview the result before treating it as complete.',
      behaviorIds: ['artifact.satisfy-template'],
      toolIds: ['template-get', 'render-preview'],
    },
    {
      id: 'skill.accessibility-review',
      name: 'accessibility-review',
      description: 'Checks an artifact for non-visual usability.',
      body: 'Review reading order, text alternatives, labels, contrast, and whether meaning survives without color or layout alone.',
      behaviorIds: ['artifact.check-accessibility'],
      toolIds: ['render-preview'],
    },
    {
      id: 'skill.information-design',
      name: 'information-design',
      description: 'Prioritizes information in a compact artifact.',
      body: 'Make the primary decision immediately visible, subordinate supporting detail, and remove visual elements that do not carry information.',
      behaviorIds: ['artifact.prioritize-information'],
      toolIds: [],
    },
    {
      id: 'skill.schema-validation',
      name: 'schema-validation',
      description: 'Validates structured artifact content.',
      body: 'Validate the entire candidate against the declared schema and treat every missing required field or unknown field as a blocking defect.',
      behaviorIds: ['artifact.validate-schema'],
      toolIds: ['schema-validate'],
    },
    {
      id: 'skill.artifact-publishing',
      name: 'artifact-publishing',
      description: 'Publishes a validated artifact.',
      body: 'Publish only after validation succeeds, preserve the returned artifact reference, and do not substitute a draft path for the published result.',
      behaviorIds: ['artifact.publish-release'],
      toolIds: ['artifact-publish', 'write'],
    },
    {
      id: 'skill.motion-design',
      name: 'motion-design',
      description: 'Adds decorative motion to presentations.',
      body: 'Use animation to guide attention between states. This procedure is not appropriate for static reports or schema-bound documents.',
      behaviorIds: ['artifact.add-animation'],
      toolIds: ['render-animation'],
    },
  ],
};

const direct = buildPlanningCase(fixture, {
  id: 'planning.artifacts.a',
  title: 'Rewrite a supplied status sentence',
  compositionClass: 'A',
  request:
    'Rewrite this sentence in plain language: “The deployment completed successfully and all monitored indicators remain within their expected ranges.” Return one sentence and create no file.',
  requestBehaviors: [
    requestBehavior(
      'artifact.preserve-stated-meaning',
      'Preserve the supplied deployment meaning in one plain sentence.',
    ),
  ],
  outputs: [
    {
      id: 'output.plain-status',
      description: 'One plain-language status sentence.',
    },
  ],
  roles: [
    {
      id: 'role.rewrite-status',
      description: 'Rewrite the supplied sentence.',
      outputIds: ['output.plain-status'],
      behaviorIds: ['artifact.preserve-stated-meaning'],
    },
  ],
  dependencies: [],
  relevantSkillIds: [],
  baseToolIds: [],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.rewrite-status',
        ['output.plain-status'],
        ['artifact.preserve-stated-meaning'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['artifact.add-animation'],
  ),
  p1: phase(
    [
      criterion(
        'role.rewrite-status',
        ['output.plain-status'],
        ['artifact.preserve-stated-meaning'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['artifact.add-animation'],
  ),
});

const baseTool = buildPlanningCase(fixture, {
  id: 'planning.artifacts.b',
  title: 'Read an opaque report template',
  compositionClass: 'B',
  request:
    'Read the base file templates/weekly-report.json, whose fields are not shown here, and list its required field names in source order. Do not render or publish anything.',
  requestBehaviors: [
    requestBehavior(
      'artifact.read-template-file',
      'Read the opaque template before naming its fields.',
    ),
    requestBehavior(
      'artifact.report-template-fields',
      'Report the required fields in their observed source order.',
    ),
  ],
  outputs: [
    {
      id: 'output.template-source',
      description: 'The inspected template source.',
    },
    {
      id: 'output.template-fields',
      description: 'The ordered required field names.',
    },
  ],
  roles: [
    {
      id: 'role.read-template',
      description: 'Read the opaque template file.',
      outputIds: ['output.template-source'],
      behaviorIds: ['artifact.read-template-file'],
    },
    {
      id: 'role.report-template',
      description: 'Report its required fields.',
      outputIds: ['output.template-fields'],
      behaviorIds: ['artifact.report-template-fields'],
    },
  ],
  dependencies: [
    dependency(
      'dependency.template-before-fields',
      'role.read-template',
      'role.report-template',
    ),
  ],
  relevantSkillIds: [],
  baseToolIds: ['read'],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.read-template',
        ['output.template-source'],
        ['artifact.read-template-file'],
      ),
      criterion(
        'role.report-template',
        ['output.template-fields'],
        ['artifact.report-template-fields'],
      ),
    ],
    ['dependency.template-before-fields'],
    { min: 2, max: 2 },
    ['artifact.add-animation'],
  ),
  p1: phase(
    [
      criterion(
        'role.read-template',
        ['output.template-source'],
        ['artifact.read-template-file'],
      ),
      criterion(
        'role.report-template',
        ['output.template-fields'],
        ['artifact.report-template-fields'],
      ),
    ],
    ['dependency.template-before-fields'],
    { min: 2, max: 2 },
    ['artifact.add-animation'],
  ),
});

const cognitive = buildPlanningCase(fixture, {
  id: 'planning.artifacts.c',
  title: 'Structure supplied release facts',
  compositionClass: 'C',
  request:
    'Using only these facts—release 2.4, faster startup, deprecated legacy login, migration guide available—produce a compact release-note outline. No file or rendering tool is needed.',
  requestBehaviors: [
    requestBehavior(
      'artifact.include-release-facts',
      'Include each supplied release fact without inventing another change.',
    ),
  ],
  outputs: [
    {
      id: 'output.release-outline',
      description: 'A compact hierarchy for the supplied release facts.',
    },
  ],
  roles: [
    {
      id: 'role.structure-release',
      description: 'Organize the supplied facts into a release-note outline.',
      outputIds: ['output.release-outline'],
      behaviorIds: [
        'artifact.include-release-facts',
        'artifact.organize-hierarchy',
      ],
    },
  ],
  dependencies: [],
  relevantSkillIds: ['skill.editorial-structure'],
  baseToolIds: [],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.structure-release',
        ['output.release-outline'],
        ['artifact.include-release-facts'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['artifact.add-animation'],
  ),
  p1: phase(
    [
      criterion(
        'role.structure-release',
        ['output.release-outline'],
        ['artifact.include-release-facts', 'artifact.organize-hierarchy'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['artifact.add-animation'],
  ),
});

const oneSkillTools = buildPlanningCase(fixture, {
  id: 'planning.artifacts.d',
  title: 'Populate and preview a fixed report template',
  compositionClass: 'D',
  request:
    'Fetch the incident-summary template, populate it with title “Cache incident” and summary “Recovered in 12 minutes”, then preview the complete candidate. Do not publish it.',
  requestBehaviors: [
    requestBehavior(
      'artifact.use-supplied-report-values',
      'Use the supplied title and summary exactly once.',
    ),
    requestBehavior(
      'artifact.preview-candidate',
      'Preview the completed candidate before finishing.',
    ),
  ],
  outputs: [
    {
      id: 'output.report-template',
      description: 'The fetched report template.',
    },
    {
      id: 'output.report-preview',
      description: 'A preview of the populated incident summary.',
    },
  ],
  roles: [
    {
      id: 'role.obtain-report-template',
      description: 'Obtain the fixed template contract.',
      outputIds: ['output.report-template'],
      behaviorIds: ['artifact.use-supplied-report-values'],
    },
    {
      id: 'role.populate-preview',
      description: 'Populate and preview a compliant candidate.',
      outputIds: ['output.report-preview'],
      behaviorIds: ['artifact.preview-candidate', 'artifact.satisfy-template'],
    },
  ],
  dependencies: [
    dependency(
      'dependency.template-before-preview',
      'role.obtain-report-template',
      'role.populate-preview',
    ),
  ],
  relevantSkillIds: ['skill.template-compliance'],
  baseToolIds: [],
  declaredToolIds: ['template-get', 'render-preview'],
  p0: phase(
    [
      criterion(
        'role.obtain-report-template',
        ['output.report-template'],
        ['artifact.use-supplied-report-values'],
      ),
      criterion(
        'role.populate-preview',
        ['output.report-preview'],
        ['artifact.preview-candidate'],
      ),
    ],
    ['dependency.template-before-preview'],
    { min: 2, max: 3 },
    ['artifact.publish-release'],
  ),
  p1: phase(
    [
      criterion(
        'role.obtain-report-template',
        ['output.report-template'],
        ['artifact.use-supplied-report-values'],
      ),
      criterion(
        'role.populate-preview',
        ['output.report-preview'],
        ['artifact.preview-candidate', 'artifact.satisfy-template'],
      ),
    ],
    ['dependency.template-before-preview'],
    { min: 2, max: 3 },
    ['artifact.publish-release'],
  ),
});

const manySkillsSimpleMenu = buildPlanningCase(fixture, {
  id: 'planning.artifacts.e',
  title: 'Design and accessibility-check a compact dashboard',
  compositionClass: 'E',
  request:
    'Prepare a one-screen dashboard specification whose primary decision is service health and whose supporting details are latency and errors. Preview it once and include accessibility acceptance criteria.',
  requestBehaviors: [
    requestBehavior(
      'artifact.cover-dashboard-facts',
      'Represent service health, latency, and errors in the specification.',
    ),
    requestBehavior(
      'artifact.preview-dashboard',
      'Preview the compact dashboard once.',
    ),
  ],
  outputs: [
    {
      id: 'output.dashboard-specification',
      description: 'An information-prioritized dashboard specification.',
    },
    {
      id: 'output.dashboard-accessibility',
      description: 'Accessibility acceptance criteria for the dashboard.',
    },
  ],
  roles: [
    {
      id: 'role.design-dashboard',
      description: 'Prioritize and specify dashboard content.',
      outputIds: ['output.dashboard-specification'],
      behaviorIds: [
        'artifact.cover-dashboard-facts',
        'artifact.preview-dashboard',
        'artifact.prioritize-information',
      ],
    },
    {
      id: 'role.review-accessibility',
      description: 'Define the accessibility acceptance criteria.',
      outputIds: ['output.dashboard-accessibility'],
      behaviorIds: ['artifact.check-accessibility'],
    },
  ],
  dependencies: [
    dependency(
      'dependency.design-before-accessibility',
      'role.design-dashboard',
      'role.review-accessibility',
    ),
  ],
  relevantSkillIds: ['skill.information-design', 'skill.accessibility-review'],
  baseToolIds: [],
  declaredToolIds: ['render-preview'],
  p0: phase(
    [
      criterion(
        'role.design-dashboard',
        ['output.dashboard-specification'],
        ['artifact.cover-dashboard-facts', 'artifact.preview-dashboard'],
      ),
      criterion(
        'role.review-accessibility',
        ['output.dashboard-accessibility'],
        [],
      ),
    ],
    ['dependency.design-before-accessibility'],
    { min: 2, max: 3 },
    ['artifact.add-animation'],
  ),
  p1: phase(
    [
      criterion(
        'role.design-dashboard',
        ['output.dashboard-specification'],
        [
          'artifact.cover-dashboard-facts',
          'artifact.preview-dashboard',
          'artifact.prioritize-information',
        ],
      ),
      criterion(
        'role.review-accessibility',
        ['output.dashboard-accessibility'],
        ['artifact.check-accessibility'],
      ),
    ],
    ['dependency.design-before-accessibility'],
    { min: 2, max: 3 },
    ['artifact.add-animation'],
  ),
});

const manySkillsManyTools = buildPlanningCase(fixture, {
  id: 'planning.artifacts.f',
  title: 'Build, validate, and publish a schema-bound report',
  compositionClass: 'F',
  request:
    'Fetch the audit-report template, populate and preview it, validate the complete candidate against its schema, then publish the validated artifact and retain its reference.',
  requestBehaviors: [
    requestBehavior(
      'artifact.build-audit-report',
      'Build and preview the requested audit report.',
    ),
    requestBehavior(
      'artifact.retain-published-reference',
      'Return the reference of the published artifact.',
    ),
  ],
  outputs: [
    {
      id: 'output.audit-candidate',
      description: 'The populated report candidate.',
    },
    {
      id: 'output.audit-validation',
      description: 'Successful full-schema validation evidence.',
    },
    {
      id: 'output.audit-reference',
      description: 'The published artifact reference.',
    },
  ],
  roles: [
    {
      id: 'role.build-audit-report',
      description: 'Fetch, populate, and preview the report.',
      outputIds: ['output.audit-candidate'],
      behaviorIds: ['artifact.build-audit-report', 'artifact.satisfy-template'],
    },
    {
      id: 'role.validate-audit-report',
      description: 'Validate the complete candidate.',
      outputIds: ['output.audit-validation'],
      behaviorIds: ['artifact.validate-schema'],
    },
    {
      id: 'role.publish-audit-report',
      description: 'Publish and retain the artifact reference.',
      outputIds: ['output.audit-reference'],
      behaviorIds: [
        'artifact.retain-published-reference',
        'artifact.publish-release',
      ],
    },
  ],
  dependencies: [
    dependency(
      'dependency.candidate-before-validation',
      'role.build-audit-report',
      'role.validate-audit-report',
    ),
    dependency(
      'dependency.validation-before-publish',
      'role.validate-audit-report',
      'role.publish-audit-report',
    ),
  ],
  relevantSkillIds: [
    'skill.template-compliance',
    'skill.schema-validation',
    'skill.artifact-publishing',
  ],
  baseToolIds: [],
  declaredToolIds: [
    'template-get',
    'render-preview',
    'schema-validate',
    'artifact-publish',
    'write',
  ],
  p0: phase(
    [
      criterion(
        'role.build-audit-report',
        ['output.audit-candidate'],
        ['artifact.build-audit-report'],
      ),
      criterion('role.validate-audit-report', ['output.audit-validation'], []),
      criterion(
        'role.publish-audit-report',
        ['output.audit-reference'],
        ['artifact.retain-published-reference'],
      ),
    ],
    [
      'dependency.candidate-before-validation',
      'dependency.validation-before-publish',
    ],
    { min: 3, max: 4 },
    ['artifact.add-animation'],
  ),
  p1: phase(
    [
      criterion(
        'role.build-audit-report',
        ['output.audit-candidate'],
        ['artifact.build-audit-report', 'artifact.satisfy-template'],
      ),
      criterion(
        'role.validate-audit-report',
        ['output.audit-validation'],
        ['artifact.validate-schema'],
      ),
      criterion(
        'role.publish-audit-report',
        ['output.audit-reference'],
        ['artifact.retain-published-reference', 'artifact.publish-release'],
      ),
    ],
    [
      'dependency.candidate-before-validation',
      'dependency.validation-before-publish',
    ],
    { min: 3, max: 4 },
    ['artifact.add-animation'],
  ),
});

export const artifactCases: readonly PlanningCase[] = [
  direct,
  baseTool,
  cognitive,
  oneSkillTools,
  manySkillsSimpleMenu,
  manySkillsManyTools,
];
