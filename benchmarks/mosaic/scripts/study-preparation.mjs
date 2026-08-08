import { join } from 'node:path';

import {
  cleanCommit,
  cli,
  execute,
  fail,
  readJson,
  writeJson,
} from './study-runtime.mjs';

const scorePath = (root) => join(root, 'scores.json');

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

const caseFile = async (context, path) =>
  (await readJson(path)).map((value) => context.api.CaseV1.parse(value));

const issue = (name, issues) => {
  if (issues.length === 0) return;
  const first = issues[0];
  fail(`${name} is invalid: ${first.code} (${first.caseId})`);
};

/** Fails before paid work when authored scientific inputs are malformed. */
export const validateAuthoredInputs = async (context) => {
  const calibrationCases = await caseFile(
    context,
    context.config.files.calibrationCases,
  );
  const confirmatoryCases = await caseFile(
    context,
    context.config.files.confirmatoryCases,
  );
  issue('calibration cases', context.api.study.validateCases(calibrationCases));
  issue(
    'calibration cases',
    context.api.study.validateFamilyIsolation(
      context.api.study.PILOT_CASES,
      calibrationCases,
    ),
  );
  if (calibrationCases.some(({ phase }) => phase !== 'calibration')) {
    fail('calibration cases must use the calibration phase');
  }
  issue(
    'confirmatory cases',
    context.api.study.validateConfirmatoryCases(
      confirmatoryCases,
      context.api.study.PILOT_CASES,
      confirmatoryCases.length,
    ),
  );
  issue(
    'calibration/confirmatory isolation',
    context.api.study.validateFamilyIsolation(
      calibrationCases,
      confirmatoryCases,
    ),
  );
  await cli({
    label: 'validate prices',
    args: ['validate', '--prices', context.config.files.prices],
    receipt: join(context.paths.preflight, 'prices.json'),
  });
  const prices = await readJson(context.config.files.prices);
  const requiredModels = [
    context.api.config.PRIMARY_MODEL.model,
    context.config.candidate.model,
    context.api.config.EMBEDDING_MODEL.model,
    context.api.config.RERANKER_MODEL,
  ];
  if (
    typeof prices.models !== 'object' ||
    prices.models === null ||
    requiredModels.some((model) => !(model in prices.models))
  ) {
    fail('prices must include the primary, candidate, embedder, and reranker');
  }
  context.cases = {
    calibration: calibrationCases,
    confirmatory: confirmatoryCases,
  };
};

export const runAndScore = async (context, phase) => {
  const common = [
    '--schedule',
    phase.schedule,
    '--artifacts',
    join(phase.root, 'artifacts'),
  ];
  const cases = phase.cases ? ['--cases', phase.cases] : [];
  const freeze = phase.freeze ? ['--freeze', phase.freeze] : [];
  await cli({
    label: `${phase.name} run`,
    args: [
      'run',
      ...common,
      ...cases,
      '--prices',
      context.config.files.prices,
      ...freeze,
      '--resume',
    ],
    receipt: join(phase.root, 'run-summary.json'),
  });
  await cli({
    label: `${phase.name} score`,
    args: [
      'score',
      ...common,
      ...cases,
      ...freeze,
      '--family',
      phase.family,
      '--output',
      scorePath(phase.root),
      '--csv',
      join(phase.root, 'scores.csv'),
    ],
    receipt: join(phase.root, 'score-command.json'),
    outputs: [scorePath(phase.root), join(phase.root, 'scores.csv')],
  });
};

export const pilot = async (context) => {
  const schedule = join(context.paths.pilot, 'schedule.json');
  await cli({
    label: 'pilot schedule',
    args: [
      'pilot',
      '--study-id',
      context.config.studyId,
      '--seed',
      context.config.seeds.pilot,
      '--output',
      schedule,
    ],
    receipt: join(context.paths.pilot, 'schedule-command.json'),
    outputs: [schedule],
  });
  await runAndScore(context, {
    name: 'pilot',
    root: context.paths.pilot,
    schedule,
    family: 'exploratory',
  });
};

export const calibration = async (context) => {
  const cases = context.cases.calibration;
  const luna = context.api.study.createSchedule({
    studyId: context.config.studyId,
    cases,
    conditions: [context.api.conditions.M1],
    seed: context.config.seeds.calibrationSchedule,
    repetitions: 3,
    capture: 'structure',
    freezeHash: null,
  });
  const candidate = luna.map((run) =>
    context.api.RunSpecV1.parse({
      ...run,
      id: `run.candidate.${context.api.core
        .contentHash({ run: run.id, model: context.config.candidate })
        .slice(0, 32)}`,
      model: context.config.candidate,
    }),
  );
  const lunaRoot = join(context.paths.calibration, 'luna');
  const candidateRoot = join(context.paths.calibration, 'candidate');
  await writeJson(context, join(lunaRoot, 'schedule.json'), luna);
  await writeJson(context, join(candidateRoot, 'schedule.json'), candidate);
  await runAndScore(context, {
    name: 'calibration Luna',
    root: lunaRoot,
    schedule: join(lunaRoot, 'schedule.json'),
    cases: context.config.files.calibrationCases,
    family: 'exploratory',
  });
  await runAndScore(context, {
    name: 'calibration candidate',
    root: candidateRoot,
    schedule: join(candidateRoot, 'schedule.json'),
    cases: context.config.files.calibrationCases,
    family: 'exploratory',
  });
  const inputPath = join(context.paths.calibration, 'input.json');
  await writeJson(context, inputPath, {
    studyId: context.config.studyId,
    candidate: context.config.candidate,
    seed: context.config.seeds.calibrationBootstrap,
    cases,
    lunaRows: await readJson(scorePath(lunaRoot)),
    candidateRows: await readJson(scorePath(candidateRoot)),
  });
  const resultPath = join(context.paths.calibration, 'calibration.json');
  await cli({
    label: 'model calibration',
    args: ['calibrate-models', '--input', inputPath, '--output', resultPath],
    receipt: join(context.paths.calibration, 'command.json'),
    outputs: [resultPath],
  });
  const result = await readJson(resultPath);
  if (result.result?.approved !== true) {
    fail('replication candidate failed calibration; freeze is blocked');
  }
};

export const power = async (context) => {
  const resultPath = join(context.paths.power, 'power-result.json');
  await cli({
    label: 'power analysis',
    args: [
      'power',
      '--config',
      context.config.files.powerConfig,
      '--image',
      context.config.analysisImage,
      '--work-dir',
      join(context.paths.power, 'verification'),
      '--result',
      resultPath,
    ],
    receipt: join(context.paths.power, 'command.json'),
    outputs: [resultPath],
  });
  return context.api.PowerResultV1.parse(await readJson(resultPath));
};

/** Creates exactly the frozen baseline/M1 pair for every confirmatory block. */
export const createPrefreezeSchedule = (context, cases, baselineId) =>
  context.api.study.createSchedule({
    studyId: context.config.studyId,
    cases,
    conditions: [
      context.api.conditions.conditionById(baselineId),
      context.api.conditions.M1,
    ],
    seed: context.config.seeds.confirmatorySchedule,
    repetitions: 5,
    capture: 'structure',
    freezeHash: null,
  });

export const prepareFreeze = async (context, powerResult) => {
  const cases = context.cases.confirmatory;
  const pilotRows = await readJson(scorePath(context.paths.pilot));
  const selection = context.api.study.selectBaseline(
    pilotRows.filter((row) =>
      ['B0', 'B1', 'B2', 'B3'].includes(row.conditionId),
    ),
  );
  const budget = context.api.modelCallBudgetAtP95(
    pilotRows,
    selection.conditionId,
  );
  const schedule = createPrefreezeSchedule(
    context,
    cases,
    selection.conditionId,
  );
  const schedulePath = join(
    context.paths.schedules,
    'confirmatory.prefreeze.json',
  );
  await writeJson(context, schedulePath, schedule);
  const validated = await cli({
    label: 'freeze inputs validation',
    args: [
      'validate',
      '--cases',
      context.config.files.confirmatoryCases,
      '--n-final',
      String(powerResult.nFinal),
      '--schedule',
      schedulePath,
      '--prices',
      context.config.files.prices,
      '--pilot-scores',
      scorePath(context.paths.pilot),
      '--calibration',
      join(context.paths.calibration, 'calibration.json'),
      '--power-config',
      context.config.files.powerConfig,
      '--power-result',
      join(context.paths.power, 'power-result.json'),
    ],
    receipt: join(context.paths.freeze, 'validate.json'),
  });
  const artifactHashes = validated.result?.artifactHashes;
  if (typeof artifactHashes !== 'object' || artifactHashes === null) {
    fail('freeze validation returned no artifact hashes');
  }
  const calibrationResult = await readJson(
    join(context.paths.calibration, 'calibration.json'),
  );
  const digest = context.config.analysisImage.includes('@')
    ? context.config.analysisImage.split('@').at(-1)
    : context.config.analysisImage;
  const inputPath = join(context.paths.freeze, 'input.json');
  await writeJson(context, inputPath, {
    schemaVersion: 1,
    studyId: context.config.studyId,
    frozenAt: context.config.frozenAt,
    gitCommit: context.commit,
    cleanWorktree: true,
    primaryModel: context.api.config.PRIMARY_MODEL,
    reranker: context.api.config.RERANKER_MODEL,
    embedder: context.api.config.EMBEDDING_MODEL,
    selectedBaseline: selection.conditionId,
    repetitions: 5,
    alpha: 0.05,
    minimumEffect: 0.1,
    targetPower: 0.8,
    nPower: powerResult.nPower,
    nFinal: powerResult.nFinal,
    modelCallBudgetP95: budget,
    semanticReviewFraction: 0.2,
    artifactHashes,
    replication: {
      candidate: calibrationResult.candidate,
      ...calibrationResult.result,
    },
    containerDigest: digest,
  });
  return { cases, schedule, schedulePath, inputPath };
};

export const freeze = async (context, prepared) => {
  if ((await cleanCommit()) !== context.commit) {
    fail('repository commit changed during study preparation');
  }
  const resultPath = join(context.paths.freeze, 'freeze.json');
  await cli({
    label: 'immutable freeze',
    args: [
      'freeze',
      '--input',
      prepared.inputPath,
      '--path',
      resultPath,
      '--pilot-scores',
      scorePath(context.paths.pilot),
      '--calibration',
      join(context.paths.calibration, 'calibration.json'),
      '--power-config',
      context.config.files.powerConfig,
      '--power-result',
      join(context.paths.power, 'power-result.json'),
      '--cases',
      context.config.files.confirmatoryCases,
      '--schedule',
      prepared.schedulePath,
      '--prices',
      context.config.files.prices,
      '--image',
      context.config.analysisImage,
    ],
    receipt: join(context.paths.freeze, 'command.json'),
    outputs: [resultPath],
  });
  return context.api.FreezeManifestV1.parse(await readJson(resultPath));
};
