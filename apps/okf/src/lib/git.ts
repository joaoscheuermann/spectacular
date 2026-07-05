import fs from 'node:fs';
import path from 'node:path';

const GIT_IGNORE_FILE_NAME = '.gitignore';

export function parseGitIgnoreFile(body: string) {
  return body.split('\n').map((item) => item.trim());
}

export const readGitIgnoreFile = async (root: string) => {
  const target = path.join(root, GIT_IGNORE_FILE_NAME);

  try {
    await fs.promises.access(target, fs.constants.F_OK);

    return parseGitIgnoreFile(await fs.promises.readFile(target, 'utf-8'));
  } catch {
    return [];
  }
};
