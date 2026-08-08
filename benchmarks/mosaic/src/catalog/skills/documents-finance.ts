import type { SkillSeed } from '../types.js';

export const DOCUMENTS_FINANCE: readonly SkillSeed[] = [
  {
    slug: 'invoice-reconcile',
    title: 'Reconcile invoices',
    description:
      'Reconcile invoice documents against ledger records and report exact discrepancies.',
    procedure: [
      'Find the invoice set.',
      'Fetch each source document.',
      'Look up matching ledger records.',
      'Calculate and report differences without changing sources.',
    ],
    tools: ['document_search', 'document_fetch', 'record_lookup', 'calculate'],
    relation: 'equivalent',
    peer: 'ledger-reconcile',
    rationale:
      'Both procedures reconcile the same evidence with a different starting point.',
  },
  {
    slug: 'ledger-reconcile',
    title: 'Reconcile ledger records',
    description:
      'Reconcile ledger records against their invoice evidence and report exact discrepancies.',
    procedure: [
      'Look up the ledger slice.',
      'Find referenced invoices.',
      'Fetch the evidence.',
      'Calculate and report mismatches without changing sources.',
    ],
    tools: ['record_lookup', 'document_search', 'document_fetch', 'calculate'],
    relation: 'equivalent',
    peer: 'invoice-reconcile',
    rationale:
      'Both procedures produce the same reconciliation from opposite indexes.',
  },
  {
    slug: 'invoice-summarize',
    title: 'Summarize invoices',
    description:
      'Summarize invoice contents without asserting that they match accounting records.',
    procedure: [
      'Find the requested invoices.',
      'Fetch their contents.',
      'Summarize totals and dates.',
      'Label unverified fields.',
    ],
    tools: ['document_search', 'document_fetch', 'calculate'],
    relation: 'overlap',
    peer: 'invoice-reconcile',
    rationale:
      'It reads the same documents but does not perform reconciliation.',
  },
  {
    slug: 'policy-search',
    title: 'Search finance policy',
    description: 'Locate finance-policy passages relevant to a named question.',
    procedure: [
      'Search policy documents.',
      'Fetch the best matches.',
      'Return the passages with document identifiers.',
    ],
    tools: ['document_search', 'document_fetch'],
    relation: 'distractor',
    peer: 'invoice-reconcile',
    rationale:
      'Finance vocabulary overlaps, but policy search cannot reconcile transactions.',
  },
  {
    slug: 'overwrite-ledger',
    title: 'Overwrite ledger from invoices',
    description:
      'Describe why ledger replacement is unsafe and require an approved reconciliation instead.',
    procedure: [
      'Do not overwrite ledger records.',
      'Identify the requested destructive assumption.',
      'Redirect to a discrepancy report.',
    ],
    tools: ['document_search', 'record_lookup'],
    relation: 'conflict',
    peer: 'invoice-reconcile',
    rationale:
      'Replacing authoritative records conflicts with evidence-preserving reconciliation.',
  },
  {
    slug: 'cashflow-trend',
    title: 'Analyze cash-flow trends',
    description:
      'Calculate period-over-period cash-flow changes from authoritative time series.',
    procedure: [
      'Query the requested series and range.',
      'Calculate comparable period changes.',
      'Report values and missing periods.',
    ],
    tools: ['timeseries_query', 'calculate'],
    relation: 'equivalent',
    peer: 'cashflow-delta',
    rationale: 'Both compute the same period changes with equivalent ordering.',
  },
  {
    slug: 'cashflow-delta',
    title: 'Calculate cash-flow deltas',
    description:
      'Calculate cash-flow deltas and rates from an authoritative time series.',
    procedure: [
      'Query all requested periods.',
      'Calculate absolute and percentage deltas.',
      'Report source periods with results.',
    ],
    tools: ['timeseries_query', 'calculate'],
    relation: 'equivalent',
    peer: 'cashflow-trend',
    rationale: 'Both compute identical change measures from the same series.',
  },
  {
    slug: 'cashflow-convert',
    title: 'Normalize cash-flow currency',
    description:
      'Convert cash-flow observations into one currency before comparison.',
    procedure: [
      'Query the cash-flow series.',
      'Convert each value using the fixture rate.',
      'Calculate and label normalized changes.',
    ],
    tools: ['timeseries_query', 'currency_convert', 'calculate'],
    relation: 'overlap',
    peer: 'cashflow-trend',
    rationale: 'It adds currency normalization to the same trend evidence.',
  },
  {
    slug: 'revenue-trend',
    title: 'Analyze revenue trends',
    description: 'Analyze revenue rather than cash-flow observations.',
    procedure: [
      'Query the revenue series.',
      'Calculate period changes.',
      'Report revenue results only.',
    ],
    tools: ['timeseries_query', 'calculate'],
    relation: 'distractor',
    peer: 'cashflow-trend',
    rationale:
      'The arithmetic is similar but the financial measure is different.',
  },
  {
    slug: 'mix-currencies',
    title: 'Avoid mixed-currency totals',
    description:
      'Detect mixed currencies and refuse to total them before normalization.',
    procedure: [
      'Inspect currency labels.',
      'Do not add unlike currencies.',
      'Request or perform explicit conversion.',
    ],
    tools: ['timeseries_query', 'currency_convert'],
    relation: 'conflict',
    peer: 'cashflow-trend',
    rationale:
      'Unnormalized aggregation conflicts with valid trend comparison.',
  },
  {
    slug: 'portfolio-exposure',
    title: 'Summarize portfolio exposure',
    description: 'Summarize portfolio positions by asset class and currency.',
    procedure: [
      'Load the portfolio snapshot.',
      'Group positions by requested dimensions.',
      'Calculate weights.',
      'Report totals and coverage.',
    ],
    tools: ['portfolio_snapshot', 'calculate'],
    relation: 'equivalent',
    peer: 'portfolio-allocation',
    rationale:
      'Exposure and allocation use the same grouping and weight calculation.',
  },
  {
    slug: 'portfolio-allocation',
    title: 'Calculate portfolio allocation',
    description:
      'Calculate portfolio allocation weights by asset class and currency.',
    procedure: [
      'Load positions.',
      'Calculate total value.',
      'Calculate grouped weights.',
      'Report the full allocation.',
    ],
    tools: ['portfolio_snapshot', 'calculate'],
    relation: 'equivalent',
    peer: 'portfolio-exposure',
    rationale: 'It is a procedural equivalent of the exposure summary.',
  },
  {
    slug: 'portfolio-concentration',
    title: 'Check portfolio concentration',
    description: 'Identify concentrated positions within a portfolio snapshot.',
    procedure: [
      'Load positions.',
      'Calculate position weights.',
      'Report weights above the stated threshold.',
    ],
    tools: ['portfolio_snapshot', 'calculate'],
    relation: 'overlap',
    peer: 'portfolio-exposure',
    rationale:
      'It shares position weights but answers a narrower risk question.',
  },
  {
    slug: 'portfolio-document',
    title: 'Find portfolio commentary',
    description:
      'Find narrative portfolio commentary without calculating current exposure.',
    procedure: [
      'Search commentary documents.',
      'Fetch the best match.',
      'Return cited narrative passages.',
    ],
    tools: ['document_search', 'document_fetch'],
    relation: 'distractor',
    peer: 'portfolio-exposure',
    rationale: 'Commentary mentions portfolios but is not position evidence.',
  },
  {
    slug: 'portfolio-double-count',
    title: 'Prevent portfolio double counting',
    description:
      'Detect duplicate positions and withhold allocation totals until they are resolved.',
    procedure: [
      'Load the snapshot.',
      'Identify duplicate position identifiers.',
      'Do not publish inflated totals.',
      'Report duplicates.',
    ],
    tools: ['portfolio_snapshot'],
    relation: 'conflict',
    peer: 'portfolio-exposure',
    rationale: 'Double counting conflicts with valid allocation weights.',
  },
];
