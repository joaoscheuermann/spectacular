import { TOOL_NAMES, type ToolName } from '../config/index.js';
import { canonicalJson, jsonSnapshot, type JsonValue } from '../core/json.js';
import { TOOL_CONTRACTS } from './contracts.js';
import type { World } from './world.js';

export interface ToolExecution {
  readonly world: World;
  readonly output: JsonValue;
}

export interface BenchmarkTool {
  readonly name: ToolName;
  readonly description: string;
  readonly input: (typeof TOOL_CONTRACTS)[ToolName]['input'];
  readonly output: (typeof TOOL_CONTRACTS)[ToolName]['output'];
  readonly execute: (world: World, input: JsonValue) => ToolExecution;
}

type Handler = (
  world: World,
  input: Readonly<Record<string, JsonValue>>,
) => ToolExecution;

const objectInput = (input: JsonValue): Readonly<Record<string, JsonValue>> => {
  if (typeof input !== 'object' || input === null || Array.isArray(input)) {
    throw new TypeError('tool input must be a JSON object');
  }
  return input as Readonly<Record<string, JsonValue>>;
};

const stringField = (
  input: Readonly<Record<string, JsonValue>>,
  key: string,
): string => {
  const value = input[key];
  if (typeof value !== 'string' || value.trim() === '') {
    throw new TypeError(`${key} must be a non-empty string`);
  }
  return value;
};

const numberField = (
  input: Readonly<Record<string, JsonValue>>,
  key: string,
): number => {
  const value = input[key];
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new TypeError(`${key} must be a finite number`);
  }
  return value;
};

const unchanged = (world: World, output: JsonValue): ToolExecution => ({
  world,
  output,
});

const writeWorld = (
  world: World,
  path: string,
  content: string,
): ToolExecution => ({
  world: { ...world, files: { ...world.files, [path]: content } },
  output: { path, bytes: Buffer.byteLength(content, 'utf8') },
});

const list: Handler = (world, input) => {
  const prefix = typeof input['prefix'] === 'string' ? input['prefix'] : '';
  return unchanged(
    world,
    Object.keys(world.files)
      .filter((path) => path.startsWith(prefix))
      .sort(),
  );
};

const read: Handler = (world, input) => {
  const path = stringField(input, 'path');
  const content = world.files[path];
  if (content === undefined) throw new Error('fixture path not found');
  return unchanged(world, { path, content });
};

const write: Handler = (world, input) =>
  writeWorld(world, stringField(input, 'path'), stringField(input, 'content'));

const search: Handler = (world, input) => {
  const query = stringField(input, 'query').toLocaleLowerCase('en-US');
  const matches = Object.entries(world.files)
    .filter(([, content]) => content.toLocaleLowerCase('en-US').includes(query))
    .map(([path]) => path)
    .sort();
  return unchanged(world, { query, matches });
};

const calculate: Handler = (world, input) => {
  const operation = stringField(input, 'operation');
  const values = input['values'];
  if (
    !Array.isArray(values) ||
    values.length === 0 ||
    !values.every(
      (value) => typeof value === 'number' && Number.isFinite(value),
    )
  ) {
    throw new TypeError('values must be a non-empty finite number array');
  }
  const numbers = values as readonly number[];
  const operations: Readonly<Record<string, () => number>> = {
    add: () => numbers.reduce((sum, value) => sum + value, 0),
    subtract: () =>
      numbers
        .slice(1)
        .reduce((result, value) => result - value, numbers[0] as number),
    multiply: () => numbers.reduce((result, value) => result * value, 1),
    divide: () =>
      numbers
        .slice(1)
        .reduce((result, value) => result / value, numbers[0] as number),
    sum: () => numbers.reduce((sum, value) => sum + value, 0),
  };
  const run = operations[operation];
  if (run === undefined) throw new TypeError('unsupported operation');
  const result = run();
  if (!Number.isFinite(result))
    throw new TypeError('calculation produced a non-finite result');
  return unchanged(world, { operation, result });
};

const jsonQuery: Handler = (world, input) => {
  const path = stringField(input, 'path');
  const query = stringField(input, 'query');
  const content = world.files[path];
  if (content === undefined) throw new Error('fixture path not found');
  let value: unknown = JSON.parse(content) as unknown;
  for (const segment of query.split('.').filter(Boolean)) {
    if (typeof value !== 'object' || value === null || !(segment in value))
      throw new Error('query path not found');
    value = (value as Record<string, unknown>)[segment];
  }
  return unchanged(world, jsonSnapshot(value as JsonValue));
};

const documentSearch: Handler = (world, input) => {
  const query = stringField(input, 'query').toLocaleLowerCase('en-US');
  const matches = Object.values(world.documents)
    .filter((document) =>
      `${document.title}\n${document.text}`
        .toLocaleLowerCase('en-US')
        .includes(query),
    )
    .map(({ id, title }) => ({ id, title }))
    .sort((left, right) => left.id.localeCompare(right.id));
  return unchanged(world, matches);
};

const documentFetch: Handler = (world, input) => {
  const id = stringField(input, 'id');
  const document = world.documents[id];
  if (document === undefined) throw new Error('document not found');
  return unchanged(world, { ...document });
};

const recordLookup: Handler = (world, input) => {
  const id = stringField(input, 'id');
  const record = world.records[id];
  if (record === undefined) throw new Error('record not found');
  return unchanged(world, { id, record });
};

const timeseriesQuery: Handler = (world, input) => {
  const series = stringField(input, 'series');
  const values = world.timeseries[series];
  if (values === undefined) throw new Error('timeseries not found');
  return unchanged(world, { series, values: [...values] });
};

const currencyConvert: Handler = (world, input) => {
  const from = stringField(input, 'from');
  const to = stringField(input, 'to');
  const amount = numberField(input, 'amount');
  const rate = from === to ? 1 : world.rates[`${from}:${to}`];
  if (rate === undefined) throw new Error('conversion rate not found');
  return unchanged(world, { from, to, amount, rate, converted: amount * rate });
};

const portfolioSnapshot: Handler = (world) =>
  unchanged(
    world,
    world.portfolio.map((position) => ({ ...position })),
  );

const symbolLookup: Handler = (world, input) => {
  const name = stringField(input, 'name');
  const symbol = world.symbols[name];
  if (symbol === undefined) throw new Error('symbol not found');
  return unchanged(world, { name, symbol });
};

const dependencyQuery: Handler = (world, input) => {
  const component = stringField(input, 'component');
  const direction =
    typeof input['direction'] === 'string'
      ? input['direction']
      : 'dependencies';
  if (!(component in world.dependencies))
    throw new Error('component not found');
  const values =
    direction === 'dependents'
      ? Object.entries(world.dependencies)
          .filter(([, dependencies]) => dependencies.includes(component))
          .map(([name]) => name)
      : [...(world.dependencies[component] ?? [])];
  return unchanged(world, { component, direction, values: values.sort() });
};

const testRun: Handler = (world, input) => {
  const target = stringField(input, 'target');
  const result = world.tests[target];
  if (result === undefined) throw new Error('test target not found');
  return unchanged(world, { target, result });
};

const buildCheck: Handler = (world, input) => {
  const target = typeof input['target'] === 'string' ? input['target'] : 'all';
  const result = world.builds[target];
  if (result === undefined) throw new Error('build target not found');
  return unchanged(world, { target, result });
};

const templateGet: Handler = (world, input) => {
  const name = stringField(input, 'name');
  const template = world.templates[name];
  if (template === undefined) throw new Error('template not found');
  return unchanged(world, { name, template });
};

const schemaValidate: Handler = (world, input) => {
  const name = stringField(input, 'name');
  const value = input['value'];
  const schema = world.schemas[name];
  if (
    schema === undefined ||
    typeof schema !== 'object' ||
    schema === null ||
    Array.isArray(schema)
  )
    throw new Error('schema not found');
  if (typeof value !== 'object' || value === null || Array.isArray(value))
    return unchanged(world, { valid: false, missing: ['object'] });
  const required = (schema as Readonly<Record<string, JsonValue>>)['required'];
  const requiredFields = Array.isArray(required)
    ? required.filter((field): field is string => typeof field === 'string')
    : [];
  const missing = requiredFields.filter((field) => !(field in value));
  return unchanged(world, { valid: missing.length === 0, missing });
};

const renderPreview: Handler = (world, input) => {
  const format = typeof input['format'] === 'string' ? input['format'] : 'json';
  const value = input['value'] ?? null;
  return unchanged(world, { format, rendered: canonicalJson(value) });
};

const artifactPublish: Handler = (world, input) => {
  const value = input['value'] ?? null;
  const next = world.counters.artifact + 1;
  const id = `artifact-${String(next).padStart(3, '0')}`;
  return {
    world: {
      ...world,
      artifacts: { ...world.artifacts, [id]: jsonSnapshot(value) },
      counters: { ...world.counters, artifact: next },
    },
    output: { id, published: true },
  };
};

const channelList: Handler = (world) =>
  unchanged(
    world,
    Object.entries(world.channels)
      .map(([name, id]) => ({ name, id }))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );

const recipientResolve: Handler = (world, input) => {
  const query = stringField(input, 'query');
  return unchanged(world, {
    query,
    candidates: [...(world.recipients[query] ?? [])],
  });
};

const messageSend: Handler = (world, input) => {
  const recipientId = stringField(input, 'recipientId');
  const channelId =
    typeof input['channelId'] === 'string' ? input['channelId'] : 'direct';
  const body = stringField(input, 'body');
  const next = world.counters.message + 1;
  const id = `msg-${String(next).padStart(3, '0')}`;
  const message = {
    id,
    recipientId,
    channelId,
    body,
    status: 'delivered' as const,
  };
  return {
    world: {
      ...world,
      messages: { ...world.messages, [id]: message },
      counters: { ...world.counters, message: next },
    },
    output: { id, status: message.status },
  };
};

const messageStatus: Handler = (world, input) => {
  const id = stringField(input, 'id');
  const message = world.messages[id];
  if (message === undefined) throw new Error('message not found');
  return unchanged(world, {
    id,
    recipientId: message.recipientId,
    status: message.status,
  });
};

const handlers: Readonly<Record<ToolName, Handler>> = {
  list,
  read,
  write,
  search,
  calculate,
  json_query: jsonQuery,
  document_search: documentSearch,
  document_fetch: documentFetch,
  record_lookup: recordLookup,
  timeseries_query: timeseriesQuery,
  currency_convert: currencyConvert,
  portfolio_snapshot: portfolioSnapshot,
  symbol_lookup: symbolLookup,
  dependency_query: dependencyQuery,
  test_run: testRun,
  build_check: buildCheck,
  template_get: templateGet,
  schema_validate: schemaValidate,
  render_preview: renderPreview,
  artifact_publish: artifactPublish,
  channel_list: channelList,
  recipient_resolve: recipientResolve,
  message_send: messageSend,
  message_status: messageStatus,
};

/** The exact 24-tool deterministic benchmark surface. */
export const TOOLS: readonly BenchmarkTool[] = TOOL_NAMES.map((name) => ({
  name,
  description: TOOL_CONTRACTS[name].description,
  input: TOOL_CONTRACTS[name].input,
  output: TOOL_CONTRACTS[name].output,
  execute: (world, input) => {
    const parsedInput = TOOL_CONTRACTS[name].input.parse(input) as JsonValue;
    const result = handlers[name](world, objectInput(parsedInput));
    return {
      world: result.world,
      output: TOOL_CONTRACTS[name].output.parse(result.output) as JsonValue,
    };
  },
}));

export const executeTool = (
  world: World,
  name: ToolName,
  input: JsonValue,
): ToolExecution => {
  const tool = TOOLS.find((candidate) => candidate.name === name);
  if (tool === undefined) throw new TypeError(`unknown tool: ${name}`);
  return tool.execute(world, input);
};
