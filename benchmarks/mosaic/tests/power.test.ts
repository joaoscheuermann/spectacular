import assert from 'node:assert/strict';
import test from 'node:test';

import {
  finalSampleSize,
  selectPoweredSampleSize,
} from '../src/scoring/index.js';

test('selects the first powered multiple of twenty-four', () => {
  assert.equal(
    selectPoweredSampleSize([
      { cases: 216, power: 0.78 },
      { cases: 240, power: 0.79 },
      { cases: 264, power: 0.81 },
      { cases: 288, power: 0.83 },
    ]),
    264,
  );
});

test('rounds the final sample to a multiple of one hundred twenty', () => {
  assert.equal(finalSampleSize(264), 360);
  assert.equal(finalSampleSize(120), 240);
  assert.equal(finalSampleSize(240), 240);
});
