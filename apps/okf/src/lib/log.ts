export type LogEntry = {
  readonly hash: string;
  readonly timestamp: string;
};

export type FolderLog = ReadonlyMap<string, readonly LogEntry[]>;

export const parseLog = (content: string): FolderLog => {
  const sections = new Map<string, LogEntry[]>();
  let currentFile: string | undefined;

  for (const line of content.split(/\r?\n/u)) {
    if (line.startsWith('## ')) {
      currentFile = line.slice(3).trim();
      sections.set(currentFile, [...(sections.get(currentFile) ?? [])]);
      continue;
    }

    if (currentFile === undefined || !line.startsWith('|')) {
      continue;
    }

    const columns = line
      .split('|')
      .slice(1, -1)
      .map((column) => column.trim().replace(/^`|`$/g, ''));

    if (
      columns.length !== 2 ||
      columns[0] === 'hash' ||
      columns[0].startsWith('---')
    ) {
      continue;
    }

    sections.set(currentFile, [
      ...(sections.get(currentFile) ?? []),
      { hash: columns[0], timestamp: columns[1] },
    ]);
  }

  return sections;
};

export const timestampForHash = (
  log: FolderLog,
  fileName: string,
  hash: string,
  fallback: string,
): string =>
  log.get(fileName)?.find((entry) => entry.hash === hash)?.timestamp ??
  fallback;

export const appendHash = (
  log: FolderLog,
  fileName: string,
  hash: string,
  timestamp: string,
): FolderLog => {
  const next = new Map(log);
  const entries = next.get(fileName) ?? [];

  if (entries.some((entry) => entry.hash === hash)) {
    return next;
  }

  next.set(fileName, [...entries, { hash, timestamp }]);

  return next;
};

export const renderLog = (
  log: FolderLog,
  currentFileNames: readonly string[],
): string => {
  const fileNames = [...new Set([...currentFileNames, ...log.keys()])].sort();
  const sections = fileNames.map((fileName) => renderFileLog(fileName, log));

  return ['# Log', '', ...sections].join('\n').trimEnd() + '\n';
};

const renderFileLog = (fileName: string, log: FolderLog): string =>
  [
    `## ${fileName}`,
    '',
    '| hash | timestamp |',
    '| --- | --- |',
    ...(log.get(fileName) ?? []).map(
      (entry) => `| \`${entry.hash}\` | ${entry.timestamp} |`,
    ),
    '',
  ].join('\n');
