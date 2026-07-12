export type ProgressEvent = {
  readonly event: string;
  readonly [key: string]: unknown;
};

export type Progress = (event: ProgressEvent) => void;
