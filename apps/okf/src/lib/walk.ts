import { glob } from 'glob';

interface WalkOptions {
  ignore?: Array<string>;
}

/** walks the file system ignoring files for the current root path */
export async function walk(
  root: string,
  callback: (root: string, target: string) => Promise<void>,
  options: WalkOptions,
): Promise<void> {
  const targets = await glob('*', {
    cwd: root,
    ignore: options.ignore ?? [],
  });

  for (const target of targets) await callback(root, target);
}
