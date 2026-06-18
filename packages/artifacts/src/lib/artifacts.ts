import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

export type Artifact<TData> = {
  readonly name: string;
  readonly mime: string;
  readonly data: TData;
  readonly template: (data: TData) => string;
};

export type SaveArtifactOptions = {
  readonly directory: string;
  readonly fileName?: string;
};

/** Renders an artifact through its template without mutating the artifact data. */
export const renderArtifact = <TData>(artifact: Artifact<TData>): string =>
  artifact.template(artifact.data);

/** Creates the destination directory, writes the rendered artifact as UTF-8, and returns the written path. */
export const saveArtifact = async <TData>(
  artifact: Artifact<TData>,
  options: SaveArtifactOptions,
): Promise<string> => {
  const fileName = options.fileName ?? defaultFileName(artifact);

  assertSafeName(fileName, 'artifact file name');
  await mkdir(options.directory, { recursive: true });

  const target = path.join(options.directory, fileName);
  await writeFile(target, renderArtifact(artifact), 'utf8');

  return target;
};

const defaultFileName = <TData>(artifact: Artifact<TData>): string => {
  assertSafeName(artifact.name, 'artifact name');

  return `${artifact.name}${extensionForMime(artifact.mime)}`;
};

const extensionForMime = (mime: string): string =>
  mime.toLowerCase() === 'text/markdown' ? '.md' : '';

const assertSafeName = (value: string, label: string): void => {
  if (
    value.trim() === '' ||
    value !== value.trim() ||
    value === '.' ||
    value === '..' ||
    value.includes('..') ||
    value.includes('/') ||
    value.includes('\\') ||
    value.includes('\0') ||
    path.isAbsolute(value) ||
    path.win32.isAbsolute(value) ||
    path.posix.isAbsolute(value) ||
    /^[a-zA-Z]:/.test(value)
  ) {
    throw new Error(`Unsafe ${label}: ${value}`);
  }
};
