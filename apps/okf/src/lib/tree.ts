import type { AnalyzedFile, DirectoryNode } from './types.js';
import { parentRelativePath } from './paths.js';

export const buildTree = (
  directoryPaths: readonly string[],
  files: readonly AnalyzedFile[],
): DirectoryNode => {
  const knownDirectories = new Set(['', ...directoryPaths]);

  for (const file of files) {
    addParents(knownDirectories, file.folderRelativePath);
  }

  return nodeFor('', knownDirectories, files);
};

/** Traverses a generated source tree in deterministic folder order. */
export const walk = async (
  node: DirectoryNode,
  visit: (node: DirectoryNode) => Promise<void> | void,
): Promise<void> => {
  await visit(node);

  for (const directory of node.directories) {
    await walk(directory, visit);
  }
};

const nodeFor = (
  relativePath: string,
  knownDirectories: ReadonlySet<string>,
  files: readonly AnalyzedFile[],
): DirectoryNode => {
  const directFiles = files
    .filter((file) => file.folderRelativePath === relativePath)
    .sort((first, second) => first.basename.localeCompare(second.basename));
  const directories = [...knownDirectories]
    .filter((directory) => parentRelativePath(directory) === relativePath)
    .filter((directory) => directory !== relativePath)
    .sort()
    .map((directory) => nodeFor(directory, knownDirectories, files));

  return {
    relativePath,
    files: directFiles,
    directories,
  };
};

const addParents = (directories: Set<string>, relativePath: string): void => {
  if (relativePath === '') {
    return;
  }

  directories.add(relativePath);
  addParents(directories, parentRelativePath(relativePath));
};
