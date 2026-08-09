import { join } from 'node:path';

import { cleanCommit, cli, execute } from './study-runtime.mjs';

/** Runs repository, instrument, container, and analysis gates before paid work. */
export const preflight = async (context) => {
  await cleanCommit();
  const gates = [
    ['sync', ['nx', 'sync']],
    ['projects', ['nx', 'show', 'projects']],
    [
      'typecheck',
      [
        'nx',
        'run-many',
        '-t',
        'typecheck',
        '-p',
        'agent,mosaic,mosaic-benchmark',
        '--parallel=2',
      ],
    ],
    [
      'build',
      [
        'nx',
        'run-many',
        '-t',
        'build',
        '-p',
        'agent,mosaic,mosaic-benchmark',
        '--parallel=1',
      ],
    ],
    [
      'test',
      [
        'nx',
        'run-many',
        '-t',
        'test',
        '-p',
        'agent,mosaic,mosaic-benchmark',
        '--parallel=1',
      ],
    ],
    ['schemas', ['nx', 'run', 'mosaic-benchmark:schemas']],
  ];
  for (const [label, args] of gates) await execute('npx', args, { label });
  await cli({
    label: 'validate instrument',
    args: ['validate'],
    receipt: join(context.paths.preflight, 'validate.json'),
  });
  await cli({
    label: 'conformance',
    args: ['conformance'],
    receipt: join(context.paths.preflight, 'conformance.json'),
  });
  await execute('git', ['diff', '--check'], { label: 'git diff check' });
  await execute('docker', ['image', 'inspect', context.config.analysisImage], {
    label: 'analysis image check',
  });
  await execute(
    'docker',
    [
      'run',
      '--rm',
      '--pull',
      'never',
      '--network',
      'none',
      '--read-only',
      '--cap-drop',
      'ALL',
      '--security-opt',
      'no-new-privileges',
      '--tmpfs',
      '/tmp:rw,noexec,nosuid,size=256m',
      '--env',
      'TMPDIR=/tmp',
      '--user',
      `${process.getuid()}:${process.getgid()}`,
      context.config.analysisImage,
      '/benchmark/analysis/test-analysis.R',
    ],
    { label: 'R container test' },
  );
  context.commit = await cleanCommit();
};
