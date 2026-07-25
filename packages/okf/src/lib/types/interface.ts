export type ImportEntry = {
  readonly source: string;
  readonly target?: string | null;
  readonly symbols: readonly string[];
};

export type ExtractedImport = {
  readonly source: string;
  readonly symbols: readonly string[];
};

export type ModuleImports = {
  readonly relative?: readonly ImportEntry[];
  readonly external?: readonly ImportEntry[];
};

export type ModuleExports = {
  readonly classes?: readonly string[];
  readonly methods?: readonly string[];
  readonly functions?: readonly string[];
  readonly variables?: readonly string[];
  readonly types?: readonly string[];
  readonly reexports?: readonly string[];
};

export type ModuleInterface = {
  readonly imports?: ModuleImports;
  readonly exports?: ModuleExports;
};
