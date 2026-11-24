// Die Kappung der Download-Zahlen (#104) an erfundenen Zahlen: Ohne Ausreißer bleibt alles,
// ein einzelner Ausreißer wird gekappt, und die Untergrenze schützt kleine Zahlen.
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { FLOOR, countCap, dampCounts, median } from '../src/lib/download-counts.mjs';

test('median of odd, even and empty lists', () => {
  assert.equal(median([3, 1, 2]), 2);
  assert.equal(median([4, 1, 3, 2]), 2.5);
  assert.equal(median([]), 0);
});

test('without an outlier every count stays as served', () => {
  const counts = [2, 3, 4, 12, 20, 0];
  assert.deepEqual(dampCounts(counts), counts);
});

test('a single extreme outlier is capped and no longer dominates', () => {
  const counts = [732, 27, 26, 26, 12, 12, 10, 3, 2, 2, 2];
  const damped = dampCounts(counts);
  const total = damped.reduce((sum, count) => sum + count, 0);
  assert.equal(damped[0], countCap(counts));
  assert.ok(damped[0] < 100, `capped to ${damped[0]}`);
  assert.ok(damped[0] / total < 0.5, 'the outlier is less than half of the total');
});

test('all zero and a single file stay unchanged', () => {
  assert.deepEqual(dampCounts([0, 0, 0]), [0, 0, 0]);
  assert.deepEqual(dampCounts([7]), [7]);
});

test('above the floor the cap is five medians', () => {
  assert.equal(countCap([10, 10, 10, 200]), 50);
  assert.deepEqual(dampCounts([10, 10, 10, 200]), [10, 10, 10, 50]);
});

test('the floor keeps small counts from being capped', () => {
  // Median 1, fünffach 5: ohne Untergrenze würde schon eine 6 gekappt.
  assert.equal(countCap([1, 1, 1, 20]), FLOOR);
  assert.deepEqual(dampCounts([1, 1, 1, 20]), [1, 1, 1, 20]);
});
