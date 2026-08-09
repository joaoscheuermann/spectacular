import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

import { CaseDraftDocumentV1, type Case } from '../src/schemas/index.js';
import { TOOL_NAMES } from '../src/config/index.js';
import { artifactHash } from '../src/core/index.js';
import {
  PILOT_CASES,
  compileCalibrationCases,
  validateCalibrationCorpus,
} from '../src/study/index.js';

const draftPath = 'benchmarks/mosaic/cases/calibration-draft.v1.json';
const corpusPath = 'benchmarks/mosaic/cases/calibration-cases.v1.json';
const reviewPath = 'benchmarks/mosaic/cases/calibration-h01-review-template.md';
const auditPath = 'benchmarks/mosaic/cases/calibration-audit.v1.json';

const readJson = async (path: string): Promise<unknown> =>
  JSON.parse(await readFile(path, 'utf8'));

const countBy = <K extends string>(
  cases: readonly Case[],
  select: (entry: Case) => K,
): Readonly<Record<K, number>> =>
  cases.reduce<Record<K, number>>(
    (counts, entry) => ({
      ...counts,
      [select(entry)]: (counts[select(entry)] ?? 0) + 1,
    }),
    {} as Record<K, number>,
  );

test('checked-in calibration draft compiles exactly to the canonical corpus', async () => {
  const draft = CaseDraftDocumentV1.parse(await readJson(draftPath));
  const checkedIn = await readJson(corpusPath);
  const compilation = compileCalibrationCases(draft, PILOT_CASES);

  assert.equal(compilation.valid, true);
  assert.deepEqual(compilation.issues, []);
  assert.deepEqual(compilation.cases, checkedIn);

  const validation = validateCalibrationCorpus(checkedIn, PILOT_CASES);
  assert.equal(validation.valid, true);
  assert.deepEqual(validation.issues, []);
  assert.deepEqual(validation.humanAuditRequired, [
    'semantic-neutrality',
    'cross-phase-independence',
  ]);
});

test('calibration corpus is balanced, independent, and marker-free', async () => {
  const draft = CaseDraftDocumentV1.parse(await readJson(draftPath));
  const cases = validateCalibrationCorpus(
    await readJson(corpusPath),
    PILOT_CASES,
  ).cases;

  assert.equal(cases.length, 60);
  assert.deepEqual(
    countBy(cases, (entry) => entry.domain),
    {
      'documents-finance': 15,
      software: 15,
      artifacts: 15,
      communication: 15,
    },
  );
  assert.deepEqual(
    countBy(cases, (entry) => entry.compositionClass),
    {
      A: 10,
      B: 10,
      C: 10,
      D: 10,
      E: 10,
      F: 10,
    },
  );
  assert.ok(cases.every((entry) => entry.phase === 'calibration'));
  assert.ok(cases.every((entry) => entry.id.startsWith('calibration.case.')));
  assert.ok(
    cases.every((entry) => entry.familyId.startsWith('calibration.family.')),
  );

  const pilotRequests = new Set(PILOT_CASES.map(({ request }) => request));
  const pilotTitles = new Set(PILOT_CASES.map(({ title }) => title));
  const pilotAnswers = new Set(
    PILOT_CASES.map((entry) =>
      artifactHash(entry.gold.expectedDelivery.document['answer']),
    ),
  );
  assert.ok(cases.every((entry) => !pilotRequests.has(entry.request)));
  assert.ok(cases.every((entry) => !pilotTitles.has(entry.title)));
  assert.ok(
    cases.every(
      (entry) =>
        !pilotAnswers.has(
          artifactHash(entry.gold.expectedDelivery.document['answer']),
        ),
    ),
  );

  const serializedDraft = JSON.stringify(draft).toLowerCase();
  assert.doesNotMatch(
    serializedDraft,
    /(?:completion|success)[-_ ]?(?:marker|token)|password|secret[-_ ]?phrase/,
  );
  assert.deepEqual(
    [...new Set(cases.flatMap((entry) => entry.gold.requiredTools))].sort(),
    [...TOOL_NAMES].sort(),
  );
});

test('H-01 approval remains bound to the reviewed calibration corpus', async () => {
  const review = await readFile(reviewPath, 'utf8');
  const audit = (await readJson(auditPath)) as Readonly<
    Record<string, unknown>
  >;
  const draftBytes = await readFile(draftPath);
  const corpusBytes = await readFile(corpusPath);
  const draft = JSON.parse(draftBytes.toString()) as unknown;
  const corpus = JSON.parse(corpusBytes.toString()) as unknown;

  assert.match(review, /Status: APPROVED — REVIEWED — H-01 ACCEPTED/);
  assert.match(review, /Approved for H-01: \*\*true\*\*/);
  assert.match(review, /Decision: \*\*APPROVED\*\*/);
  assert.doesNotMatch(review, /\[ \]|☐|\| pending \|/);
  assert.match(review, new RegExp(artifactHash(draft)));
  assert.match(review, new RegExp(artifactHash(corpus)));
  assert.match(
    review,
    new RegExp(createHash('sha256').update(draftBytes).digest('hex')),
  );
  assert.match(
    review,
    new RegExp(createHash('sha256').update(corpusBytes).digest('hex')),
  );
  assert.equal(
    [...review.matchAll(/\| `calibration\.case\.[a-f]\d{2}` \|/g)].length,
    60,
  );
  assert.deepEqual(Object.keys(audit).sort(), [
    'approvedAt',
    'approvedBy',
    'artifactHash',
    'checks',
    'rationale',
    'schemaVersion',
    'subject',
  ]);
  assert.equal(audit['schemaVersion'], 1);
  assert.equal(audit['subject'], 'calibration-corpus');
  assert.equal(audit['artifactHash'], artifactHash(corpus));
  assert.equal(
    audit['approvedBy'],
    'João Vitor Scheuermann <joao.s@wonderful.ai>',
  );
  assert.ok(Number.isFinite(new Date(String(audit['approvedAt'])).valueOf()));
  assert.deepEqual(audit['checks'], [
    'english',
    'neutrality',
    'difficulty',
    'no-answer-marker',
    'procedural-equivalence',
    'overlap',
    'distractors',
    'conflicts',
    'phase-isolation',
  ]);
});
