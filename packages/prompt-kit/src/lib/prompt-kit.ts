import { AsyncLocalStorage } from 'node:async_hooks';
import { stdin, stdout } from 'node:process';

import chalk, { Chalk } from 'chalk';

import type {
  Choice,
  PromptAnswers,
  PromptInput,
  PromptIo,
  PromptKit,
  PromptOutput,
  PromptTask,
} from './types/prompt.js';

type ResolvedPromptIo = {
  readonly input: PromptInput;
  readonly output: PromptOutput;
};

type QueueContext = {
  readonly io: ResolvedPromptIo;
  readonly prefix: (title: string) => string;
};

type LineState = {
  buffer: string;
  ended: boolean;
  listening: boolean;
  resolve?: (line: string) => void;
  reject?: (error: Error) => void;
};

const queueContext = new AsyncLocalStorage<QueueContext>();
const lineStates = new WeakMap<PromptInput, LineState>();
const titleChalk = new Chalk({ level: 1 });

/** Creates a prompt API bound to the provided input and output streams. */
export const createPromptKit = (io?: PromptIo): PromptKit => {
  const fallbackIo = resolveIo(io);

  return {
    text: (question) => promptText(currentIo(fallbackIo), prefixed(question)),
    select: (title, description, choices) =>
      promptSelect(currentIo(fallbackIo), prefixed(title), description, choices),
    queue: async (tasks) => runQueue(fallbackIo, tasks),
  };
};

const runQueue = async <T extends readonly PromptTask[]>(
  io: ResolvedPromptIo,
  tasks: T,
): Promise<PromptAnswers<T>> => {
  const answers: unknown[] = [];

  for (const [index, task] of tasks.entries()) {
    const prefix = (title: string): string =>
      `[${index + 1}/${tasks.length}] ${title}`;
    const answer = await queueContext.run({ io, prefix }, task);
    answers.push(answer);
  }

  return answers as PromptAnswers<T>;
};

const promptText = async (
  io: ResolvedPromptIo,
  question: string,
): Promise<string> => {
  io.output.write(`${formatTitle(io.output, question)}\n> `);

  return readLine(io.input);
};

const promptSelect = async <T>(
  io: ResolvedPromptIo,
  title: string,
  description: string,
  choices: readonly Choice<T>[],
): Promise<T> => {
  if (choices.length === 0) {
    throw new Error('Select prompt requires at least one choice.');
  }

  if (isInteractive(io)) {
    return promptInteractiveSelect(io, title, description, choices);
  }

  return promptFallbackSelect(io, title, description, choices);
};

const promptFallbackSelect = async <T>(
  io: ResolvedPromptIo,
  title: string,
  description: string,
  choices: readonly Choice<T>[],
): Promise<T> => {
  writeFallbackSelect(io.output, title, description, choices);

  while (true) {
    const answer = (await readLine(io.input)).trim();
    const choice = choiceFromAnswer(answer, choices);

    if (choice !== undefined) {
      return choice.value;
    }

    io.output.write('Invalid choice. Enter a key or number.\n> ');
  }
};

const promptInteractiveSelect = async <T>(
  io: ResolvedPromptIo,
  title: string,
  description: string,
  choices: readonly Choice<T>[],
): Promise<T> => {
  let selected = 0;
  const lineCount = 1 + (description === '' ? 0 : 1) + choices.length;
  const render = (clear: boolean): void => {
    if (clear) {
      io.output.write(`\u001b[${lineCount}F\u001b[0J`);
    }

    io.output.write(
      [
        formatTitle(io.output, title),
        ...(description === '' ? [] : [description]),
        ...choices.map((choice, index) =>
          index === selected
            ? chalk.inverse(`> ${choice.key}`)
            : `  ${choice.key}`,
        ),
      ].join('\n') + '\n',
    );
  };

  io.input.setRawMode?.(true);
  render(false);

  try {
    while (true) {
      const key = await readKey(io.input);

      if (key === '\u0003') {
        throw new Error('Prompt cancelled.');
      }

      if (key === '\r' || key === '\n') {
        io.output.write('\n');
        return choices[selected].value;
      }

      if (key === '\u001b[A' || key === '\u001bOA') {
        selected = selected === 0 ? choices.length - 1 : selected - 1;
        render(true);
      }

      if (key === '\u001b[B' || key === '\u001bOB') {
        selected = selected === choices.length - 1 ? 0 : selected + 1;
        render(true);
      }
    }
  } finally {
    io.input.setRawMode?.(false);
  }
};

const writeFallbackSelect = <T>(
  output: PromptOutput,
  title: string,
  description: string,
  choices: readonly Choice<T>[],
): void => {
  output.write(`${formatTitle(output, title)}\n`);

  if (description !== '') {
    output.write(`${description}\n`);
  }

  for (const [index, choice] of choices.entries()) {
    output.write(`  ${index + 1}. ${choice.key}\n`);
  }

  output.write('> ');
};

const choiceFromAnswer = <T>(
  answer: string,
  choices: readonly Choice<T>[],
): Choice<T> | undefined => {
  const byKey = choices.find((choice) => choice.key === answer);

  if (byKey !== undefined) {
    return byKey;
  }

  const number = Number(answer);

  if (Number.isInteger(number) && number >= 1 && number <= choices.length) {
    return choices[number - 1];
  }

  return undefined;
};

const readLine = (input: PromptInput): Promise<string> =>
  new Promise((resolve, reject) => {
    const state = stateFor(input);
    const buffered = shiftLine(state);

    if (buffered !== undefined) {
      resolve(buffered);
      return;
    }

    if (state.ended) {
      const remaining = shiftRemaining(state);

      if (remaining !== undefined) {
        resolve(remaining);
        return;
      }

      reject(promptInputClosedError());
      return;
    }

    if (state.resolve !== undefined) {
      reject(new Error('Prompt input is already waiting for a line.'));
      return;
    }

    state.resolve = resolve;
    state.reject = reject;
    input.resume?.();
  });

const readKey = (input: PromptInput): Promise<string> =>
  new Promise((resolve) => {
    const onData = (chunk: Buffer | string): void => {
      input.removeListener('data', onData);
      resolve(chunk.toString('utf8'));
    };

    input.resume?.();
    input.on('data', onData);
  });

const stateFor = (input: PromptInput): LineState => {
  const existing = lineStates.get(input);

  if (existing !== undefined) {
    return existing;
  }

  const state: LineState = {
    buffer: '',
    ended: false,
    listening: false,
  };
  lineStates.set(input, state);
  listenForLines(input, state);

  return state;
};

const listenForLines = (input: PromptInput, state: LineState): void => {
  if (state.listening) {
    return;
  }

  state.listening = true;
  input.on('data', (chunk: Buffer | string) => {
    state.buffer += chunk.toString('utf8');
    const line = shiftLine(state);

    if (line === undefined || state.resolve === undefined) {
      return;
    }

    const resolve = state.resolve;
    state.resolve = undefined;
    state.reject = undefined;
    resolve(line);
  });

  input.on('error', (error: Error) => {
    state.reject?.(error);
    state.resolve = undefined;
    state.reject = undefined;
  });

  const close = (): void => {
    if (state.ended) {
      return;
    }

    state.ended = true;

    if (state.resolve === undefined) {
      return;
    }

    const resolve = state.resolve;
    const reject = state.reject;
    state.resolve = undefined;
    state.reject = undefined;

    const remaining = shiftRemaining(state);

    if (remaining !== undefined) {
      resolve(remaining);
      return;
    }

    reject?.(promptInputClosedError());
  };

  input.on('end', close);
  input.on('close', close);
};

const shiftLine = (state: LineState): string | undefined => {
  const match = /[\r\n]/u.exec(state.buffer);

  if (match === null) {
    return undefined;
  }

  const line = state.buffer.slice(0, match.index);
  const separatorLength =
    state.buffer[match.index] === '\r' && state.buffer[match.index + 1] === '\n'
      ? 2
      : 1;
  state.buffer = state.buffer.slice(match.index + separatorLength);

  return line;
};

const shiftRemaining = (state: LineState): string | undefined => {
  if (state.buffer.length === 0) {
    return undefined;
  }

  const line = state.buffer;
  state.buffer = '';

  return line;
};

const promptInputClosedError = (): Error =>
  new Error('Prompt input closed before a line was read.');

const formatTitle = (output: PromptOutput, title: string): string =>
  output.isTTY === true ? titleChalk.bold.cyan(title) : title;

const prefixed = (title: string): string => queueContext.getStore()?.prefix(title) ?? title;

const currentIo = (fallback: ResolvedPromptIo): ResolvedPromptIo =>
  queueContext.getStore()?.io ?? fallback;

const resolveIo = (io?: PromptIo): ResolvedPromptIo => ({
  input: io?.input ?? stdin,
  output: io?.output ?? stdout,
});

const isInteractive = (io: ResolvedPromptIo): boolean =>
  io.input.isTTY === true &&
  io.output.isTTY === true &&
  typeof io.input.setRawMode === 'function';

const defaultKit = createPromptKit();

/** Reads one line of user input without the trailing newline. */
export const text = (question: string): Promise<string> => defaultKit.text(question);

/** Prompts for one selected choice and returns the selected value. */
export const select = <T>(
  title: string,
  description: string,
  choices: readonly Choice<T>[],
): Promise<T> => defaultKit.select(title, description, choices);

/** Runs prompt tasks sequentially and returns answers in task order. */
export const queue = <T extends readonly PromptTask[]>(
  tasks: T,
): Promise<PromptAnswers<T>> => defaultKit.queue(tasks);
