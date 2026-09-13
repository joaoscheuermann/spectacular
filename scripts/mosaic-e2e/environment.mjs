import { readdir, readFile, writeFile } from 'node:fs/promises';
import { join, posix } from 'node:path';

import { createDockerClient } from 'docker';
import { createSandbox } from 'sandbox';

const uploadDirectory = async (sandbox, source, relative = '') => {
  const entries = await readdir(join(source, relative), {
    withFileTypes: true,
  });

  for (const entry of entries) {
    const path = posix.join(relative, entry.name);

    if (entry.isDirectory()) {
      const result = await sandbox.exec({ cmd: ['mkdir', '-p', path] });

      if (result.exitCode !== 0)
        {throw new Error('Cannot create sandbox input directory.');}

      await uploadDirectory(sandbox, source, path);
    } else if (entry.isFile()) {
      await sandbox.putFile(path, await readFile(join(source, path)));
    } else {
      throw new Error(
        'Task inputs must be regular files or directories: ' + path,
      );
    }
  }
};

const exportWorkspace = async (sandbox, output) => {
  const archive = await sandbox.exec({
    cmd: ['tar', '-cf', '-', '.'],
    timeoutMs: 60_000,
  });

  if (archive.exitCode !== 0)
    {throw new Error('Cannot export sandbox workspace.');}

  await writeFile(join(output, 'workspace.tar'), archive.stdoutBytes);
};

/** One container per run; preserve its files before cleanup, including failed runs. */
export const withSandbox = async ({ workspace, output, config }, execute) => {
  const sandbox = await createSandbox({
    provider: createDockerClient(),
    ...config.sandbox,
    root: '/workspace',
    imagePullPolicy: 'if-not-present',
    network: { mode: 'disabled' },
  });

  try {
    await uploadDirectory(sandbox, workspace);

    return await execute(sandbox);
  } finally {
    try {
      await exportWorkspace(sandbox, output);
    } finally {
      await sandbox.dispose();
    }
  }
};
