export type OkfToolOptions = {
  readonly workspaceRoot: string;
};

export type OkfSearchResult = {
  readonly bundle: string;
  readonly conceptId: string;
  readonly path: string;
  readonly type: string;
  readonly title: string;
  readonly description?: string;
  readonly resource?: string;
  readonly tags: readonly string[];
  readonly timestamp?: string;
  readonly content: string;
  readonly contentTruncated: boolean;
  readonly score: number;
};

export type OkfSearchOutput = {
  readonly results: readonly OkfSearchResult[];
  readonly total: number;
  readonly truncated: boolean;
  readonly skipped: number;
  readonly error?: string;
};
