import type { PlanningCase } from '../planning-schema.js';
import {
  buildPlanningCase,
  catalogBehavior,
  criterion,
  dependency,
  phase,
  requestBehavior,
  type DomainFixture,
} from './shared.js';

const fixture: DomainFixture = {
  domain: 'software',
  behaviors: [
    catalogBehavior(
      'software.check-compatibility',
      'Compare old and new contracts at the call-site boundary.',
    ),
    catalogBehavior(
      'software.trace-dependencies',
      'Trace direct and transitive dependencies before proposing a change.',
    ),
    catalogBehavior(
      'software.bound-blast-radius',
      'Identify affected consumers and unaffected boundaries.',
    ),
    catalogBehavior(
      'software.isolate-root-cause',
      'Distinguish the first causal failure from downstream symptoms.',
    ),
    catalogBehavior(
      'software.define-regression-tests',
      'Add a regression check that fails for the original defect.',
    ),
    catalogBehavior(
      'software.stage-migration',
      'Order migration steps so compatibility is preserved between stages.',
    ),
    catalogBehavior(
      'software.verify-build',
      'Run focused tests before the broader build verification.',
    ),
    catalogBehavior(
      'software.tune-performance',
      'Profile runtime hotspots before optimizing them.',
    ),
  ],
  skills: [
    {
      id: 'skill.compatibility-analysis',
      name: 'compatibility-analysis',
      description: 'Compares two software contracts.',
      body: 'Compare inputs, outputs, and failure behavior at each affected call site. Mark an incompatibility only when the new contract breaks a required old behavior.',
      behaviorIds: ['software.check-compatibility'],
      toolIds: [],
    },
    {
      id: 'skill.dependency-impact',
      name: 'dependency-impact',
      description: 'Finds the impact surface of a component change.',
      body: 'Trace direct and transitive dependents, then separate affected consumers from boundaries that do not depend on the changed contract.',
      behaviorIds: [
        'software.trace-dependencies',
        'software.bound-blast-radius',
      ],
      toolIds: ['dependency-query'],
    },
    {
      id: 'skill.root-cause-analysis',
      name: 'root-cause-analysis',
      description: 'Separates causal failures from symptoms.',
      body: 'Reproduce the failure, identify the earliest divergent observation, and do not treat later cascading failures as independent causes.',
      behaviorIds: ['software.isolate-root-cause'],
      toolIds: ['test-run'],
    },
    {
      id: 'skill.regression-testing',
      name: 'regression-testing',
      description: 'Turns a defect into a durable test.',
      body: 'Define a focused test that fails for the original defect, passes after the repair, and avoids coupling to unrelated implementation details.',
      behaviorIds: ['software.define-regression-tests'],
      toolIds: ['test-run'],
    },
    {
      id: 'skill.migration-planning',
      name: 'migration-planning',
      description: 'Orders a compatibility-preserving migration.',
      body: 'Stage adapters, consumer updates, and cleanup so every intermediate state remains buildable and rollback points stay explicit.',
      behaviorIds: ['software.stage-migration'],
      toolIds: ['write'],
    },
    {
      id: 'skill.build-verification',
      name: 'build-verification',
      description: 'Verifies a change from narrow to broad scope.',
      body: 'Run the focused regression target first and only then the affected build. Preserve both results as separate evidence.',
      behaviorIds: ['software.verify-build'],
      toolIds: ['test-run', 'build-check'],
    },
    {
      id: 'skill.performance-tuning',
      name: 'performance-tuning',
      description: 'Optimizes measured runtime bottlenecks.',
      body: 'Collect a runtime profile, select the dominant hotspot, and compare performance before and after the smallest viable optimization.',
      behaviorIds: ['software.tune-performance'],
      toolIds: ['profiler'],
    },
  ],
};

const direct = buildPlanningCase(fixture, {
  id: 'planning.software.a',
  title: 'Explain an explicit module relationship',
  compositionClass: 'A',
  request:
    'The prompt states that module api imports core and exports reconcile. Explain that relationship in one concise paragraph without inspecting the repository or proposing a migration.',
  requestBehaviors: [
    requestBehavior(
      'software.report-given-relationship',
      'Restate only the supplied import and export relationship.',
    ),
  ],
  outputs: [
    {
      id: 'output.module-explanation',
      description: 'A concise explanation of the supplied module relationship.',
    },
  ],
  roles: [
    {
      id: 'role.explain-module',
      description: 'Explain the already supplied relationship.',
      outputIds: ['output.module-explanation'],
      behaviorIds: ['software.report-given-relationship'],
    },
  ],
  dependencies: [],
  relevantSkillIds: [],
  baseToolIds: [],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.explain-module',
        ['output.module-explanation'],
        ['software.report-given-relationship'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['software.stage-migration', 'software.tune-performance'],
  ),
  p1: phase(
    [
      criterion(
        'role.explain-module',
        ['output.module-explanation'],
        ['software.report-given-relationship'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['software.stage-migration', 'software.tune-performance'],
  ),
});

const baseTool = buildPlanningCase(fixture, {
  id: 'planning.software.b',
  title: 'Inspect an opaque build configuration',
  compositionClass: 'B',
  request:
    'Read the base workspace file config/build-target.json, whose contents are not included here, and report its target and runtime. Do not redesign the build.',
  requestBehaviors: [
    requestBehavior(
      'software.read-build-config',
      'Read the opaque build configuration before reporting it.',
    ),
    requestBehavior(
      'software.report-build-config',
      'Report the observed target and runtime without redesigning them.',
    ),
  ],
  outputs: [
    {
      id: 'output.build-config',
      description: 'The inspected build configuration.',
    },
    {
      id: 'output.build-config-answer',
      description: 'The target and runtime from the configuration.',
    },
  ],
  roles: [
    {
      id: 'role.inspect-build-config',
      description: 'Inspect the opaque configuration.',
      outputIds: ['output.build-config'],
      behaviorIds: ['software.read-build-config'],
    },
    {
      id: 'role.report-build-config',
      description: 'Report the observed configuration.',
      outputIds: ['output.build-config-answer'],
      behaviorIds: ['software.report-build-config'],
    },
  ],
  dependencies: [
    dependency(
      'dependency.config-before-answer',
      'role.inspect-build-config',
      'role.report-build-config',
    ),
  ],
  relevantSkillIds: [],
  baseToolIds: ['read'],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.inspect-build-config',
        ['output.build-config'],
        ['software.read-build-config'],
      ),
      criterion(
        'role.report-build-config',
        ['output.build-config-answer'],
        ['software.report-build-config'],
      ),
    ],
    ['dependency.config-before-answer'],
    { min: 2, max: 2 },
    ['software.tune-performance'],
  ),
  p1: phase(
    [
      criterion(
        'role.inspect-build-config',
        ['output.build-config'],
        ['software.read-build-config'],
      ),
      criterion(
        'role.report-build-config',
        ['output.build-config-answer'],
        ['software.report-build-config'],
      ),
    ],
    ['dependency.config-before-answer'],
    { min: 2, max: 2 },
    ['software.tune-performance'],
  ),
});

const cognitive = buildPlanningCase(fixture, {
  id: 'planning.software.c',
  title: 'Assess a supplied API contract change',
  compositionClass: 'C',
  request:
    'The old function accepts a string and returns null when absent. The proposed function accepts a string array and throws when absent. Assess the compatibility risks from these supplied contracts; no repository inspection is needed.',
  requestBehaviors: [
    requestBehavior(
      'software.identify-contract-change',
      'Identify the changed input and missing-value behavior.',
    ),
  ],
  outputs: [
    {
      id: 'output.compatibility-assessment',
      description: 'An assessment of the supplied API compatibility risks.',
    },
  ],
  roles: [
    {
      id: 'role.assess-compatibility',
      description: 'Compare the old and proposed contracts.',
      outputIds: ['output.compatibility-assessment'],
      behaviorIds: [
        'software.identify-contract-change',
        'software.check-compatibility',
      ],
    },
  ],
  dependencies: [],
  relevantSkillIds: ['skill.compatibility-analysis'],
  baseToolIds: [],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.assess-compatibility',
        ['output.compatibility-assessment'],
        ['software.identify-contract-change'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['software.tune-performance'],
  ),
  p1: phase(
    [
      criterion(
        'role.assess-compatibility',
        ['output.compatibility-assessment'],
        ['software.identify-contract-change', 'software.check-compatibility'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['software.tune-performance'],
  ),
});

const oneSkillTools = buildPlanningCase(fixture, {
  id: 'planning.software.d',
  title: 'Bound the impact of a component change',
  compositionClass: 'D',
  request:
    'Determine which consumers are affected if component auth changes its token result type. Use the declared dependency query and produce an impact note that separates affected and unaffected boundaries.',
  requestBehaviors: [
    requestBehavior(
      'software.inspect-consumers',
      'Inspect consumers of the changed component.',
    ),
    requestBehavior(
      'software.report-impact',
      'Produce an impact note for the token result change.',
    ),
  ],
  outputs: [
    {
      id: 'output.consumer-graph',
      description: 'The relevant dependency graph.',
    },
    {
      id: 'output.impact-note',
      description: 'Affected and unaffected consumer boundaries.',
    },
  ],
  roles: [
    {
      id: 'role.inspect-consumers',
      description: 'Inspect consumers and dependencies.',
      outputIds: ['output.consumer-graph'],
      behaviorIds: [
        'software.inspect-consumers',
        'software.trace-dependencies',
      ],
    },
    {
      id: 'role.bound-impact',
      description: 'Bound and report the change impact.',
      outputIds: ['output.impact-note'],
      behaviorIds: ['software.report-impact', 'software.bound-blast-radius'],
    },
  ],
  dependencies: [
    dependency(
      'dependency.graph-before-impact',
      'role.inspect-consumers',
      'role.bound-impact',
    ),
  ],
  relevantSkillIds: ['skill.dependency-impact'],
  baseToolIds: [],
  declaredToolIds: ['dependency-query'],
  p0: phase(
    [
      criterion(
        'role.inspect-consumers',
        ['output.consumer-graph'],
        ['software.inspect-consumers'],
      ),
      criterion(
        'role.bound-impact',
        ['output.impact-note'],
        ['software.report-impact'],
      ),
    ],
    ['dependency.graph-before-impact'],
    { min: 2, max: 3 },
    ['software.tune-performance'],
  ),
  p1: phase(
    [
      criterion(
        'role.inspect-consumers',
        ['output.consumer-graph'],
        ['software.inspect-consumers', 'software.trace-dependencies'],
      ),
      criterion(
        'role.bound-impact',
        ['output.impact-note'],
        ['software.report-impact', 'software.bound-blast-radius'],
      ),
    ],
    ['dependency.graph-before-impact'],
    { min: 2, max: 3 },
    ['software.tune-performance'],
  ),
});

const manySkillsSimpleMenu = buildPlanningCase(fixture, {
  id: 'planning.software.e',
  title: 'Diagnose and lock down a regression',
  compositionClass: 'E',
  request:
    'A focused test reports an authorization failure followed by five cascading API failures. Diagnose the primary defect and define one regression test using the existing test runner.',
  requestBehaviors: [
    requestBehavior(
      'software.use-failure-order',
      'Use the observed failure order when diagnosing the defect.',
    ),
    requestBehavior(
      'software.produce-regression-plan',
      'Produce a focused regression-test plan.',
    ),
  ],
  outputs: [
    { id: 'output.root-cause', description: 'The primary failure diagnosis.' },
    {
      id: 'output.regression-test',
      description: 'A focused test for the diagnosed defect.',
    },
  ],
  roles: [
    {
      id: 'role.diagnose-failure',
      description: 'Identify the primary defect.',
      outputIds: ['output.root-cause'],
      behaviorIds: [
        'software.use-failure-order',
        'software.isolate-root-cause',
      ],
    },
    {
      id: 'role.define-regression',
      description: 'Define a durable focused test.',
      outputIds: ['output.regression-test'],
      behaviorIds: [
        'software.produce-regression-plan',
        'software.define-regression-tests',
      ],
    },
  ],
  dependencies: [
    dependency(
      'dependency.diagnosis-before-regression',
      'role.diagnose-failure',
      'role.define-regression',
    ),
  ],
  relevantSkillIds: ['skill.root-cause-analysis', 'skill.regression-testing'],
  baseToolIds: [],
  declaredToolIds: ['test-run'],
  p0: phase(
    [
      criterion(
        'role.diagnose-failure',
        ['output.root-cause'],
        ['software.use-failure-order'],
      ),
      criterion(
        'role.define-regression',
        ['output.regression-test'],
        ['software.produce-regression-plan'],
      ),
    ],
    ['dependency.diagnosis-before-regression'],
    { min: 2, max: 3 },
    ['software.tune-performance'],
  ),
  p1: phase(
    [
      criterion(
        'role.diagnose-failure',
        ['output.root-cause'],
        ['software.use-failure-order', 'software.isolate-root-cause'],
      ),
      criterion(
        'role.define-regression',
        ['output.regression-test'],
        [
          'software.produce-regression-plan',
          'software.define-regression-tests',
        ],
      ),
    ],
    ['dependency.diagnosis-before-regression'],
    { min: 2, max: 3 },
    ['software.tune-performance'],
  ),
});

const manySkillsManyTools = buildPlanningCase(fixture, {
  id: 'planning.software.f',
  title: 'Plan and verify a staged API migration',
  compositionClass: 'F',
  request:
    'Inspect dependents of api-v1, write a staged migration plan to api-v2, and verify the focused tests and affected build. Preserve compatibility until every consumer is moved.',
  requestBehaviors: [
    requestBehavior(
      'software.inspect-migration-scope',
      'Inspect the dependent consumers before planning the migration.',
    ),
    requestBehavior(
      'software.persist-migration-plan',
      'Persist the staged migration plan.',
    ),
    requestBehavior(
      'software.verify-migration',
      'Verify focused tests and the affected build.',
    ),
  ],
  outputs: [
    { id: 'output.migration-scope', description: 'The affected consumer set.' },
    {
      id: 'output.migration-plan',
      description: 'A persisted compatibility-preserving migration plan.',
    },
    {
      id: 'output.migration-verification',
      description: 'Focused-test and affected-build results.',
    },
  ],
  roles: [
    {
      id: 'role.inspect-migration',
      description: 'Inspect and bound the migration scope.',
      outputIds: ['output.migration-scope'],
      behaviorIds: [
        'software.inspect-migration-scope',
        'software.trace-dependencies',
        'software.bound-blast-radius',
      ],
    },
    {
      id: 'role.plan-migration',
      description: 'Order and persist the migration.',
      outputIds: ['output.migration-plan'],
      behaviorIds: [
        'software.persist-migration-plan',
        'software.stage-migration',
      ],
    },
    {
      id: 'role.verify-migration',
      description: 'Verify the staged result.',
      outputIds: ['output.migration-verification'],
      behaviorIds: ['software.verify-migration', 'software.verify-build'],
    },
  ],
  dependencies: [
    dependency(
      'dependency.scope-before-plan',
      'role.inspect-migration',
      'role.plan-migration',
    ),
    dependency(
      'dependency.plan-before-verification',
      'role.plan-migration',
      'role.verify-migration',
    ),
  ],
  relevantSkillIds: [
    'skill.dependency-impact',
    'skill.migration-planning',
    'skill.build-verification',
  ],
  baseToolIds: [],
  declaredToolIds: ['dependency-query', 'write', 'test-run', 'build-check'],
  p0: phase(
    [
      criterion(
        'role.inspect-migration',
        ['output.migration-scope'],
        ['software.inspect-migration-scope'],
      ),
      criterion(
        'role.plan-migration',
        ['output.migration-plan'],
        ['software.persist-migration-plan'],
      ),
      criterion(
        'role.verify-migration',
        ['output.migration-verification'],
        ['software.verify-migration'],
      ),
    ],
    ['dependency.scope-before-plan', 'dependency.plan-before-verification'],
    { min: 3, max: 4 },
    ['software.tune-performance'],
  ),
  p1: phase(
    [
      criterion(
        'role.inspect-migration',
        ['output.migration-scope'],
        [
          'software.inspect-migration-scope',
          'software.trace-dependencies',
          'software.bound-blast-radius',
        ],
      ),
      criterion(
        'role.plan-migration',
        ['output.migration-plan'],
        ['software.persist-migration-plan', 'software.stage-migration'],
      ),
      criterion(
        'role.verify-migration',
        ['output.migration-verification'],
        ['software.verify-migration', 'software.verify-build'],
      ),
    ],
    ['dependency.scope-before-plan', 'dependency.plan-before-verification'],
    { min: 3, max: 4 },
    ['software.tune-performance'],
  ),
});

export const softwareCases: readonly PlanningCase[] = [
  direct,
  baseTool,
  cognitive,
  oneSkillTools,
  manySkillsSimpleMenu,
  manySkillsManyTools,
];
