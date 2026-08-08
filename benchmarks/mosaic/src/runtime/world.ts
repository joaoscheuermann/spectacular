import { artifactHash } from '../core/hash.js';
import { jsonSnapshot, type JsonValue } from '../core/json.js';

export interface FinanceDocument {
  readonly id: string;
  readonly title: string;
  readonly text: string;
}

export interface Position {
  readonly id: string;
  readonly assetClass: string;
  readonly currency: string;
  readonly value: number;
}

export interface Message {
  readonly id: string;
  readonly recipientId: string;
  readonly channelId: string;
  readonly body: string;
  readonly status: 'delivered';
}

export interface World {
  readonly fixtureId: 'world-v1';
  readonly files: Readonly<Record<string, string>>;
  readonly documents: Readonly<Record<string, FinanceDocument>>;
  readonly records: Readonly<Record<string, JsonValue>>;
  readonly timeseries: Readonly<Record<string, readonly number[]>>;
  readonly rates: Readonly<Record<string, number>>;
  readonly portfolio: readonly Position[];
  readonly symbols: Readonly<Record<string, JsonValue>>;
  readonly dependencies: Readonly<Record<string, readonly string[]>>;
  readonly tests: Readonly<Record<string, JsonValue>>;
  readonly builds: Readonly<Record<string, JsonValue>>;
  readonly templates: Readonly<Record<string, JsonValue>>;
  readonly schemas: Readonly<Record<string, JsonValue>>;
  readonly artifacts: Readonly<Record<string, JsonValue>>;
  readonly channels: Readonly<Record<string, string>>;
  readonly recipients: Readonly<Record<string, readonly string[]>>;
  readonly messages: Readonly<Record<string, Message>>;
  readonly counters: Readonly<{ artifact: number; message: number }>;
}

const WORLD_V1: World = {
  fixtureId: 'world-v1',
  files: {
    'notes/request.md': 'Pilot request evidence for world-v1.',
    'data/settings.json': '{"currency":"USD","threshold":0.25}',
    'src/api.ts': 'export const reconcile = (left, right) => left === right;',
  },
  documents: {
    'invoice-001': {
      id: 'invoice-001',
      title: 'Invoice 001',
      text: 'Ledger record ledger-001. Total USD 125.',
    },
    'policy-001': {
      id: 'policy-001',
      title: 'Finance policy',
      text: 'Reconciliations preserve source records.',
    },
    'commentary-001': {
      id: 'commentary-001',
      title: 'Portfolio commentary',
      text: 'Diversification remains the stated objective.',
    },
  },
  records: {
    'ledger-001': { invoiceId: 'invoice-001', currency: 'USD', total: 120 },
    'ledger-002': { invoiceId: 'invoice-002', currency: 'EUR', total: 80 },
  },
  timeseries: {
    cashflow: [100, 115, 110, 140],
    revenue: [180, 200, 220, 230],
  },
  rates: {
    'USD:BRL': 5,
    'BRL:USD': 0.2,
    'EUR:USD': 1.1,
    'USD:EUR': 0.9090909091,
  },
  portfolio: [
    { id: 'position-001', assetClass: 'equity', currency: 'USD', value: 60 },
    { id: 'position-002', assetClass: 'bond', currency: 'USD', value: 25 },
    { id: 'position-003', assetClass: 'cash', currency: 'EUR', value: 15 },
  ],
  symbols: {
    reconcile: {
      file: 'src/api.ts',
      exported: true,
      signature: '(left: number, right: number) => boolean',
    },
    internalCache: {
      file: 'src/cache.ts',
      exported: false,
      signature: 'Map<string, string>',
    },
  },
  dependencies: {
    api: ['core'],
    core: ['types'],
    cli: ['api'],
    tests: ['api'],
    types: [],
  },
  tests: {
    api: { status: 'passed', tests: 4, failures: 0 },
    core: { status: 'failed', tests: 3, failures: 1 },
  },
  builds: {
    all: { status: 'passed', outputs: ['dist/api.js'] },
    api: { status: 'passed', outputs: ['dist/api.js'] },
  },
  templates: {
    report: { type: 'report', required: ['title', 'summary'] },
    dashboard: { type: 'dashboard', required: ['title', 'metrics'] },
    manifest: { type: 'manifest', required: ['name', 'entries'] },
    inventory: { type: 'inventory', required: ['items'] },
  },
  schemas: {
    report: { required: ['title', 'summary'] },
    dashboard: { required: ['title', 'metrics'] },
    manifest: { required: ['name', 'entries'] },
    inventory: { required: ['items'] },
  },
  artifacts: {},
  channels: {
    general: 'channel-general',
    finance: 'channel-finance',
    engineering: 'channel-engineering',
  },
  recipients: {
    alice: ['person-alice'],
    finance: ['channel-finance'],
    team: ['channel-general', 'channel-engineering'],
  },
  messages: {
    'msg-000': {
      id: 'msg-000',
      recipientId: 'person-alice',
      channelId: 'direct',
      body: 'Existing fixture message.',
      status: 'delivered',
    },
  },
  counters: { artifact: 0, message: 0 },
};

/** Returns a detached world so runs can never share mutations. */
export const createWorld = (): World =>
  jsonSnapshot(WORLD_V1 as unknown as JsonValue) as unknown as World;

export const worldHash = (world: World): string => artifactHash(world);
