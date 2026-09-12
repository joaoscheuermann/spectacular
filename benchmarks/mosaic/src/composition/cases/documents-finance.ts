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
  domain: 'documents-finance',
  behaviors: [
    catalogBehavior(
      'doc.calculate-variance',
      'Calculate the signed and absolute variance before interpreting it.',
    ),
    catalogBehavior(
      'doc.explain-variance',
      'Separate the observed variance from evidence-backed explanations.',
    ),
    catalogBehavior(
      'doc.apply-materiality',
      'Compare a variance with the stated materiality threshold.',
    ),
    catalogBehavior(
      'doc.reconcile-records',
      'Match records by stable identifiers before comparing their values.',
    ),
    catalogBehavior(
      'doc.preserve-provenance',
      'Retain the source identifier for every financial fact used.',
    ),
    catalogBehavior(
      'doc.structure-report',
      'Structure the report as evidence, finding, and disposition.',
    ),
    catalogBehavior(
      'doc.forecast-cashflow',
      'Project future cash flow from a historical time series.',
    ),
  ],
  skills: [
    {
      id: 'skill.variance-analysis',
      name: 'variance-analysis',
      description: 'Interprets differences between financial values.',
      body: 'Calculate signed and absolute differences first. Explain only drivers supported by the supplied evidence and distinguish observation from hypothesis.',
      behaviorIds: ['doc.calculate-variance', 'doc.explain-variance'],
      toolIds: [],
    },
    {
      id: 'skill.materiality-assessment',
      name: 'materiality-assessment',
      description: 'Evaluates a finding against an explicit threshold.',
      body: 'Compare the measured variance with the stated threshold and state the decision rule without inventing a different threshold.',
      behaviorIds: ['doc.apply-materiality'],
      toolIds: [],
    },
    {
      id: 'skill.invoice-reconciliation',
      name: 'invoice-reconciliation',
      description: 'Reconciles invoices with ledger records.',
      body: 'Fetch both records, match their stable identifiers, and only then calculate the monetary difference and record an exception.',
      behaviorIds: ['doc.reconcile-records', 'doc.calculate-variance'],
      toolIds: ['document-fetch', 'record-lookup'],
    },
    {
      id: 'skill.source-triangulation',
      name: 'source-triangulation',
      description: 'Combines financial sources without losing provenance.',
      body: 'Keep each fact associated with its source identifier and call out disagreements before using the evidence downstream.',
      behaviorIds: ['doc.preserve-provenance'],
      toolIds: ['document-fetch', 'record-lookup'],
    },
    {
      id: 'skill.financial-reporting',
      name: 'financial-reporting',
      description: 'Produces auditable financial findings.',
      body: 'Write the final artifact with separate evidence, finding, and disposition sections so a reviewer can trace the conclusion.',
      behaviorIds: ['doc.structure-report'],
      toolIds: ['write'],
    },
    {
      id: 'skill.cashflow-forecasting',
      name: 'cashflow-forecasting',
      description: 'Forecasts future cash positions.',
      body: 'Query a historical time series and apply an explicit forecasting horizon. Do not use this procedure for a point-in-time reconciliation.',
      behaviorIds: ['doc.forecast-cashflow'],
      toolIds: ['timeseries-query'],
    },
  ],
};

const direct = buildPlanningCase(fixture, {
  id: 'planning.documents-finance.a',
  title: 'Explain supplied invoice totals',
  compositionClass: 'A',
  request:
    'An invoice has subtotal USD 120, tax USD 12, and total USD 132. Return a two-sentence explanation using only those supplied values; do not forecast or apply a materiality threshold.',
  requestBehaviors: [
    requestBehavior(
      'doc.report-given-values',
      'Report the supplied subtotal, tax, and total without adding facts.',
    ),
  ],
  outputs: [
    {
      id: 'output.invoice-summary',
      description: 'A two-sentence explanation of the supplied invoice total.',
    },
  ],
  roles: [
    {
      id: 'role.summarize-invoice',
      description: 'Explain the already supplied invoice arithmetic.',
      outputIds: ['output.invoice-summary'],
      behaviorIds: ['doc.report-given-values'],
    },
  ],
  dependencies: [],
  relevantSkillIds: [],
  baseToolIds: [],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.summarize-invoice',
        ['output.invoice-summary'],
        ['doc.report-given-values'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['doc.forecast-cashflow'],
  ),
  p1: phase(
    [
      criterion(
        'role.summarize-invoice',
        ['output.invoice-summary'],
        ['doc.report-given-values'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['doc.forecast-cashflow'],
  ),
});

const baseTool = buildPlanningCase(fixture, {
  id: 'planning.documents-finance.b',
  title: 'Read an opaque account snapshot',
  compositionClass: 'B',
  request:
    'Use the base record lookup to inspect account acct-042, whose contents are not in this request. Return its recorded currency and balance, citing the account identifier.',
  requestBehaviors: [
    requestBehavior(
      'doc.inspect-account',
      'Inspect the opaque account snapshot before making a balance claim.',
    ),
    requestBehavior(
      'doc.report-account',
      'Report currency and balance with the account identifier.',
    ),
  ],
  outputs: [
    {
      id: 'output.account-evidence',
      description: 'The inspected account snapshot.',
    },
    {
      id: 'output.account-answer',
      description: 'The account currency and balance tied to acct-042.',
    },
  ],
  roles: [
    {
      id: 'role.inspect-account',
      description: 'Inspect the opaque account state.',
      outputIds: ['output.account-evidence'],
      behaviorIds: ['doc.inspect-account'],
    },
    {
      id: 'role.report-account',
      description: 'Report the observed account values.',
      outputIds: ['output.account-answer'],
      behaviorIds: ['doc.report-account'],
    },
  ],
  dependencies: [
    dependency(
      'dependency.account-before-answer',
      'role.inspect-account',
      'role.report-account',
    ),
  ],
  relevantSkillIds: [],
  baseToolIds: ['record-lookup'],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.inspect-account',
        ['output.account-evidence'],
        ['doc.inspect-account'],
      ),
      criterion(
        'role.report-account',
        ['output.account-answer'],
        ['doc.report-account'],
      ),
    ],
    ['dependency.account-before-answer'],
    { min: 2, max: 2 },
    ['doc.forecast-cashflow'],
  ),
  p1: phase(
    [
      criterion(
        'role.inspect-account',
        ['output.account-evidence'],
        ['doc.inspect-account'],
      ),
      criterion(
        'role.report-account',
        ['output.account-answer'],
        ['doc.report-account'],
      ),
    ],
    ['dependency.account-before-answer'],
    { min: 2, max: 2 },
    ['doc.forecast-cashflow'],
  ),
});

const cognitive = buildPlanningCase(fixture, {
  id: 'planning.documents-finance.c',
  title: 'Interpret a supplied quarterly variance',
  compositionClass: 'C',
  request:
    'Q1 operating expense was USD 100,000 and Q2 was USD 130,000. Interpret the change from the supplied figures and distinguish the measured change from any possible explanation. No tool call is needed.',
  requestBehaviors: [
    requestBehavior(
      'doc.identify-change',
      'Identify that operating expense increased between Q1 and Q2.',
    ),
  ],
  outputs: [
    {
      id: 'output.variance-interpretation',
      description: 'An evidence-bounded interpretation of the Q1-to-Q2 change.',
    },
  ],
  roles: [
    {
      id: 'role.interpret-variance',
      description: 'Measure and interpret the supplied variance.',
      outputIds: ['output.variance-interpretation'],
      behaviorIds: [
        'doc.identify-change',
        'doc.calculate-variance',
        'doc.explain-variance',
      ],
    },
  ],
  dependencies: [],
  relevantSkillIds: ['skill.variance-analysis'],
  baseToolIds: [],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.interpret-variance',
        ['output.variance-interpretation'],
        ['doc.identify-change'],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['doc.forecast-cashflow'],
  ),
  p1: phase(
    [
      criterion(
        'role.interpret-variance',
        ['output.variance-interpretation'],
        [
          'doc.identify-change',
          'doc.calculate-variance',
          'doc.explain-variance',
        ],
      ),
    ],
    [],
    { min: 1, max: 1 },
    ['doc.forecast-cashflow'],
  ),
});

const oneSkillTools = buildPlanningCase(fixture, {
  id: 'planning.documents-finance.d',
  title: 'Reconcile one invoice and ledger entry',
  compositionClass: 'D',
  request:
    'Reconcile invoice inv-017 against ledger entry led-017 using their declared source operations. Produce an exception record with the matched identifiers and monetary difference.',
  requestBehaviors: [
    requestBehavior(
      'doc.obtain-records',
      'Obtain both named records before producing an exception.',
    ),
    requestBehavior(
      'doc.report-exception',
      'Report the matched identifiers and monetary difference.',
    ),
  ],
  outputs: [
    {
      id: 'output.reconciliation-records',
      description: 'Both source records.',
    },
    {
      id: 'output.reconciliation-exception',
      description:
        'A reconciliation exception with identifiers and difference.',
    },
  ],
  roles: [
    {
      id: 'role.obtain-records',
      description: 'Obtain the invoice and ledger records.',
      outputIds: ['output.reconciliation-records'],
      behaviorIds: ['doc.obtain-records'],
    },
    {
      id: 'role.reconcile-records',
      description: 'Match, compare, and report the records.',
      outputIds: ['output.reconciliation-exception'],
      behaviorIds: [
        'doc.report-exception',
        'doc.reconcile-records',
        'doc.calculate-variance',
      ],
    },
  ],
  dependencies: [
    dependency(
      'dependency.records-before-reconciliation',
      'role.obtain-records',
      'role.reconcile-records',
    ),
  ],
  relevantSkillIds: ['skill.invoice-reconciliation'],
  baseToolIds: [],
  declaredToolIds: ['document-fetch', 'record-lookup'],
  p0: phase(
    [
      criterion(
        'role.obtain-records',
        ['output.reconciliation-records'],
        ['doc.obtain-records'],
      ),
      criterion(
        'role.reconcile-records',
        ['output.reconciliation-exception'],
        ['doc.report-exception'],
      ),
    ],
    ['dependency.records-before-reconciliation'],
    { min: 2, max: 3 },
  ),
  p1: phase(
    [
      criterion(
        'role.obtain-records',
        ['output.reconciliation-records'],
        ['doc.obtain-records'],
      ),
      criterion(
        'role.reconcile-records',
        ['output.reconciliation-exception'],
        [
          'doc.report-exception',
          'doc.reconcile-records',
          'doc.calculate-variance',
        ],
      ),
    ],
    ['dependency.records-before-reconciliation'],
    { min: 2, max: 3 },
  ),
});

const manySkillsSimpleMenu = buildPlanningCase(fixture, {
  id: 'planning.documents-finance.e',
  title: 'Assess a material variance',
  compositionClass: 'E',
  request:
    'A reported variance is USD 9,000 and the approved materiality threshold is USD 5,000. Prepare a decision note that measures the variance, applies the threshold, and limits explanations to supplied evidence. No external state is needed.',
  requestBehaviors: [
    requestBehavior(
      'doc.compare-threshold',
      'Reach a materiality decision using the supplied threshold.',
    ),
  ],
  outputs: [
    {
      id: 'output.materiality-note',
      description: 'A variance and materiality decision note.',
    },
  ],
  roles: [
    {
      id: 'role.assess-materiality',
      description: 'Measure, interpret, and classify the variance.',
      outputIds: ['output.materiality-note'],
      behaviorIds: [
        'doc.compare-threshold',
        'doc.calculate-variance',
        'doc.explain-variance',
        'doc.apply-materiality',
      ],
    },
  ],
  dependencies: [],
  relevantSkillIds: ['skill.variance-analysis', 'skill.materiality-assessment'],
  baseToolIds: [],
  declaredToolIds: [],
  p0: phase(
    [
      criterion(
        'role.assess-materiality',
        ['output.materiality-note'],
        ['doc.compare-threshold'],
      ),
    ],
    [],
    { min: 1, max: 2 },
    ['doc.forecast-cashflow'],
  ),
  p1: phase(
    [
      criterion(
        'role.assess-materiality',
        ['output.materiality-note'],
        [
          'doc.compare-threshold',
          'doc.calculate-variance',
          'doc.explain-variance',
          'doc.apply-materiality',
        ],
      ),
    ],
    [],
    { min: 1, max: 2 },
    ['doc.forecast-cashflow'],
  ),
});

const manySkillsManyTools = buildPlanningCase(fixture, {
  id: 'planning.documents-finance.f',
  title: 'Publish a provenance-preserving reconciliation report',
  compositionClass: 'F',
  request:
    'Obtain invoice inv-204 and ledger record led-204 from their declared sources, preserve the provenance of each fact, and publish a Markdown reconciliation report with the finding and disposition.',
  requestBehaviors: [
    requestBehavior(
      'doc.gather-sources',
      'Obtain both named sources before drawing a conclusion.',
    ),
    requestBehavior(
      'doc.publish-report',
      'Persist the requested Markdown reconciliation report.',
    ),
  ],
  outputs: [
    {
      id: 'output.source-records',
      description: 'The invoice and ledger record.',
    },
    {
      id: 'output.verified-evidence',
      description: 'Source-associated evidence ready for reporting.',
    },
    {
      id: 'output.published-report',
      description: 'The persisted Markdown reconciliation report.',
    },
  ],
  roles: [
    {
      id: 'role.gather-sources',
      description: 'Obtain the two source records.',
      outputIds: ['output.source-records'],
      behaviorIds: ['doc.gather-sources'],
    },
    {
      id: 'role.verify-provenance',
      description: 'Associate each fact with its source.',
      outputIds: ['output.verified-evidence'],
      behaviorIds: ['doc.preserve-provenance'],
    },
    {
      id: 'role.publish-financial-report',
      description: 'Structure and persist the report.',
      outputIds: ['output.published-report'],
      behaviorIds: ['doc.publish-report', 'doc.structure-report'],
    },
  ],
  dependencies: [
    dependency(
      'dependency.sources-before-report',
      'role.gather-sources',
      'role.publish-financial-report',
    ),
    dependency(
      'dependency.sources-before-verification',
      'role.gather-sources',
      'role.verify-provenance',
    ),
    dependency(
      'dependency.verification-before-report',
      'role.verify-provenance',
      'role.publish-financial-report',
    ),
  ],
  relevantSkillIds: ['skill.source-triangulation', 'skill.financial-reporting'],
  baseToolIds: [],
  declaredToolIds: ['document-fetch', 'record-lookup', 'write'],
  p0: phase(
    [
      criterion(
        'role.gather-sources',
        ['output.source-records'],
        ['doc.gather-sources'],
      ),
      criterion(
        'role.publish-financial-report',
        ['output.published-report'],
        ['doc.publish-report'],
      ),
    ],
    ['dependency.sources-before-report'],
    { min: 2, max: 3 },
  ),
  p1: phase(
    [
      criterion(
        'role.gather-sources',
        ['output.source-records'],
        ['doc.gather-sources'],
      ),
      criterion(
        'role.verify-provenance',
        ['output.verified-evidence'],
        ['doc.preserve-provenance'],
      ),
      criterion(
        'role.publish-financial-report',
        ['output.published-report'],
        ['doc.publish-report', 'doc.structure-report'],
      ),
    ],
    [
      'dependency.sources-before-report',
      'dependency.sources-before-verification',
      'dependency.verification-before-report',
    ],
    { min: 3, max: 4 },
  ),
});

export const documentFinanceCases: readonly PlanningCase[] = [
  direct,
  baseTool,
  cognitive,
  oneSkillTools,
  manySkillsSimpleMenu,
  manySkillsManyTools,
];
