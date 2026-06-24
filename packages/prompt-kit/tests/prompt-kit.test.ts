import assert from 'node:assert/strict';
import { PassThrough } from 'node:stream';
import test from 'node:test';

import {
  createPromptKit,
  select,
  type Choice,
  type PromptInput,
} from '../src/index.js';

const branchChoices = [
  { key: 'main', value: 'main' },
  { key: 'develop', value: 'develop' },
] as const satisfies readonly Choice<string>[];

const ansiPattern = /\u001b\[[0-?]*[ -/]*[@-~]/u;
const ansiPatternGlobal = /\u001b\[[0-?]*[ -/]*[@-~]/gu;

test('text returns one input line without the trailing newline', async () => {
  const { input, output, io } = createIo();
  const kit = createPromptKit(io);
  const answer = kit.text('Your name?');

  input.write('Ada Lovelace\n');

  assert.equal(await answer, 'Ada Lovelace');
  assert.equal(output.join(''), 'Your name?\n> ');
});

test('text styles the question when output is TTY', async () => {
  const { input, output, io } = createIo({ outputIsTTY: true });
  const kit = createPromptKit(io);
  const answer = kit.text('Your name?');

  input.write('Ada Lovelace\n');

  assert.equal(await answer, 'Ada Lovelace');

  const rendered = output.join('');
  const firstLine = rendered.split('\n')[0] ?? '';

  assert.match(firstLine, ansiPattern);
  assert.equal(stripAnsi(firstLine), 'Your name?');
  assert.equal(stripAnsi(rendered), 'Your name?\n> ');
});

test('select fallback accepts a choice key', async () => {
  const { input, io } = createIo();
  const kit = createPromptKit(io);
  const answer = kit.select('Branch', 'Choose a branch', branchChoices);

  input.write('develop\n');

  assert.equal(await answer, 'develop');
});

test('select fallback accepts a one-based number', async () => {
  const { input, io } = createIo();
  const kit = createPromptKit(io);
  const answer = kit.select('Branch', 'Choose a branch', branchChoices);

  input.write('2\n');

  assert.equal(await answer, 'develop');
});

test('select fallback styles the title when output is TTY', async () => {
  const { input, output, io } = createIo({ outputIsTTY: true });
  const kit = createPromptKit(io);
  const answer = kit.select('Branch', 'Choose a branch', branchChoices);

  input.write('1\n');

  assert.equal(await answer, 'main');

  const rendered = output.join('');
  const firstLine = rendered.split('\n')[0] ?? '';

  assert.match(firstLine, ansiPattern);
  assert.equal(stripAnsi(firstLine), 'Branch');
  assert.match(stripAnsi(rendered), /^Branch\nChoose a branch/u);
});

test('select interactive styles the title when output is TTY', async () => {
  const rawModeStates: boolean[] = [];
  const { input, output, io } = createIo({
    inputIsTTY: true,
    outputIsTTY: true,
    setRawMode: (enabled) => rawModeStates.push(enabled),
  });
  const kit = createPromptKit(io);
  const answer = kit.select('Branch', 'Choose a branch', branchChoices);

  input.write('\r');

  assert.equal(await answer, 'main');

  const rendered = output.join('');
  const firstLine = rendered.split('\n')[0] ?? '';

  assert.match(firstLine, ansiPattern);
  assert.equal(stripAnsi(firstLine), 'Branch');
  assert.match(stripAnsi(rendered), /^Branch\nChoose a branch\n> main/u);
  assert.deepEqual(rawModeStates, [true, false]);
});

test('select fallback retries after invalid input', async () => {
  const { input, output, io } = createIo();
  const kit = createPromptKit(io);
  const answer = kit.select('Branch', 'Choose a branch', branchChoices);

  input.write('missing\n2\n');

  assert.equal(await answer, 'develop');
  assert.match(output.join(''), /Invalid choice/u);
});

test('select fallback rejects when input closes before a choice', async () => {
  const { input, io } = createIo();
  const kit = createPromptKit(io);
  const answer = kit.select('Branch', 'Choose a branch', branchChoices);

  input.end();

  await assert.rejects(answer, /Prompt input closed/u);
});

test('text returns an unterminated line when input closes', async () => {
  const { input, io } = createIo();
  const kit = createPromptKit(io);
  const answer = kit.text('Your name?');

  input.end('Ada Lovelace');

  assert.equal(await answer, 'Ada Lovelace');
});

test('queue runs tasks sequentially and returns answers in order', async () => {
  const { io } = createIo();
  const kit = createPromptKit(io);
  const order: string[] = [];

  const answers = await kit.queue([
    () => {
      order.push('first');
      return 'one' as const;
    },
    () => {
      order.push('second');
      return 2 as const;
    },
  ] as const);

  assert.deepEqual(order, ['first', 'second']);
  assert.deepEqual(answers, ['one', 2]);
});

test('queue prefixes prompt titles and routes direct select calls through context', async () => {
  const { input, output, io } = createIo();
  const kit = createPromptKit(io);
  const answers = kit.queue([
    () => select('First', '', branchChoices),
    () => select('Second', '', branchChoices),
  ] as const);

  input.write('main\ndevelop\n');

  assert.deepEqual(await answers, ['main', 'develop']);
  assert.match(output.join(''), /\[1\/2\] First/u);
  assert.match(output.join(''), /\[2\/2\] Second/u);
});

const stripAnsi = (text: string): string => text.replace(ansiPatternGlobal, '');

const createIo = (
  options: {
    readonly inputIsTTY?: boolean;
    readonly outputIsTTY?: boolean;
    readonly setRawMode?: PromptInput['setRawMode'];
  } = {},
): {
  readonly input: PassThrough;
  readonly output: string[];
  readonly io: Parameters<typeof createPromptKit>[0];
} => {
  const input = new PassThrough() as PassThrough & PromptInput;
  const output: string[] = [];

  Object.defineProperty(input, 'isTTY', {
    value: options.inputIsTTY ?? false,
  });
  input.setRawMode = options.setRawMode;

  return {
    input,
    output,
    io: {
      input,
      output: {
        isTTY: options.outputIsTTY ?? false,
        write(text) {
          output.push(text);
        },
      },
    },
  };
};
