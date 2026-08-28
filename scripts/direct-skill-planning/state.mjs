import { randomUUID } from 'node:crypto';
import { mkdir, rename, unlink, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';

const timestamp = () => new Date().toISOString();

export const createRun = (config) => {
  const now = timestamp();
  return {
    id: randomUUID(),
    status: 'running',
    createdAt: now,
    updatedAt: now,
    config,
    providerFailures: [],
    results: [],
  };
};

export const checkpointRun = (
  run,
  results,
  providerFailures,
  status = run.status,
  failure,
) => ({
  ...run,
  status,
  updatedAt: timestamp(),
  providerFailures,
  results,
  ...(failure === undefined ? {} : { failure }),
});

export const saveRun = async (path, run) => {
  await mkdir(dirname(path), { recursive: true });
  const temporary = `${path}.${process.pid}.${randomUUID()}.tmp`;

  try {
    await writeFile(temporary, `${JSON.stringify(run, null, 2)}\n`, {
      encoding: 'utf8',
      flag: 'wx',
      mode: 0o600,
    });
    await rename(temporary, path);
  } catch (error) {
    await unlink(temporary).catch(() => undefined);
    throw error;
  }
};

export const createCheckpointWriter = (path, initialRun) => {
  const providerFailures = [];
  const results = [];
  let run = initialRun;
  let writes = Promise.resolve();

  const write = (status, failure) => {
    writes = writes.then(async () => {
      run = checkpointRun(
        run,
        results.filter((result) => result !== undefined),
        [...providerFailures],
        status,
        failure,
      );
      await saveRun(path, run);
      return run;
    });
    return writes;
  };

  return {
    async recordProviderFailure(failure) {
      const event = {
        sequence: providerFailures.length + 1,
        timestamp: timestamp(),
        ...failure,
      };
      providerFailures.push(event);
      await write(run.status);
      return event;
    },
    saveResult(index, result) {
      results[index] = result;
      return write(run.status);
    },
    finish(status, failure) {
      return write(status, failure);
    },
  };
};
