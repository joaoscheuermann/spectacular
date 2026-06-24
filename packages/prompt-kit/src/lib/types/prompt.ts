export type Choice<T> = {
  readonly key: string;
  readonly value: T;
};

export type PromptTask<T = unknown> = () => T | Promise<T>;

export type PromptAnswer<T> = T extends PromptTask<infer Answer>
  ? Awaited<Answer>
  : never;

export type PromptAnswers<T extends readonly PromptTask[]> = {
  readonly [Index in keyof T]: PromptAnswer<T[Index]>;
};

export type PromptInput = NodeJS.ReadableStream & {
  readonly isTTY?: boolean;
  setRawMode?(enabled: boolean): void;
  resume?(): void;
  pause?(): void;
};

export type PromptOutput = {
  readonly isTTY?: boolean;
  write(text: string): unknown;
};

export type PromptIo = {
  readonly input?: PromptInput;
  readonly output?: PromptOutput;
};

export type PromptKit = {
  text(question: string): Promise<string>;
  select<T>(
    title: string,
    description: string,
    choices: readonly Choice<T>[],
  ): Promise<T>;
  queue<T extends readonly PromptTask[]>(tasks: T): Promise<PromptAnswers<T>>;
};
