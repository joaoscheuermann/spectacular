import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { promisify } from 'node:util';
import test from 'node:test';

const executeFile = promisify(execFile);
const script = 'benchmarks/mosaic/scripts/cases.mjs';

const draftFrom = (entry, index) => ({
  schemaVersion: 1,
  id: `calibration.script.case.${index + 1}`,
  familyId: `calibration.script.family.${index + 1}`,
  phase: 'calibration',
  title: `Script calibration ${index + 1}: ${entry.title}`,
  domain: entry.domain,
  compositionClass: entry.compositionClass,
  focusGoalRole: entry.focusGoalRole,
  adaptive: entry.adaptive,
  request: `${entry.request}\n\nIndependent script scenario ${index + 1}.`,
  fixtureIds: entry.fixtureIds,
  tags: [
    'calibration',
    `domain.${entry.domain}`,
    `class.${entry.compositionClass}`,
  ],
  criteria: entry.gold.criteria,
  requiredSkills: entry.gold.requiredSkills,
  relevantSkills: entry.gold.relevantSkills,
  forbiddenSkills: entry.gold.forbiddenSkills,
  tools: entry.gold.expectedState.toolEvidence.map(({ id, name, input }) => ({
    id,
    name,
    input,
  })),
  expectedAnswer: entry.gold.expectedDelivery.document.answer,
  requiresRevision: entry.gold.requiresRevision,
});

test('compiles validates and refuses to overwrite a calibration corpus', async (context) => {
  const directory = await mkdtemp(join(tmpdir(), 'mosaic-cases-script-'));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const api = await import('../dist/src/index.js');
  const draft = join(directory, 'draft.json');
  const output = join(directory, 'calibration.json');
  await writeFile(
    draft,
    `${JSON.stringify({
      schemaVersion: 1,
      cases: api.study.PILOT_CASES.map(draftFrom),
    })}\n`,
  );

  const compiled = await executeFile(process.execPath, [
    script,
    'compile',
    '--draft',
    draft,
    '--output',
    output,
  ]);
  const compileReport = JSON.parse(compiled.stdout);
  assert.equal(compileReport.valid, true);
  assert.equal(compileReport.written, true);
  assert.equal(compileReport.caseCount, 60);
  assert.deepEqual(compileReport.issues, []);

  const validated = await executeFile(process.execPath, [
    script,
    'validate',
    '--calibration-cases',
    output,
  ]);
  const validationReport = JSON.parse(validated.stdout);
  assert.equal(validationReport.valid, true);
  assert.equal(validationReport.caseCount, 60);

  await assert.rejects(
    executeFile(process.execPath, [
      script,
      'compile',
      '--draft',
      draft,
      '--output',
      output,
    ]),
    (error) => {
      const report = JSON.parse(error.stdout);
      assert.equal(report.valid, false);
      assert.equal(report.issues[0].code, 'output_exists');
      return true;
    },
  );
});
