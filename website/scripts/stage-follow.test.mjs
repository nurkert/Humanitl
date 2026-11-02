// Tests des Morph-Folgers der Bühne (src/scripts/stage/follow.mjs): Bei keinem Scrolltempo darf
// die Bühne in einem Bild springen, und sie kommt immer genau am Ziel an.
//
//   node --test scripts/
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { followStep } from '../src/scripts/stage/follow.mjs';

const clamp = (v) => Math.min(1, Math.max(0, v));

/** Fährt ein Scrollprofil y(t) mit 60 Bildern je Sekunde; liefert den größten Schritt je Bild. */
function drive(profile, range, frames = 240) {
  const s = { m: 0, v: 0 };
  let maxStep = 0;
  let moving = true;
  for (let f = 0; f < frames; f++) {
    const before = s.m;
    moving = followStep(s, clamp(profile(f / 60) / range), 1 / 60);
    maxStep = Math.max(maxStep, Math.abs(s.m - before));
  }
  return { maxStep, m: s.m, moving };
}

// Wurf auf dem Telefon: Startgeschwindigkeit v0 in px/s, Abklingen wie iOS (0,998 je ms).
const fling = (v0) => (t) => {
  const k = Math.log(1 / 0.998) * 1000;
  return (v0 / k) * (1 - Math.exp(-k * t));
};

test('ein schneller Wurf über den ganzen Morph springt in keinem Bild', () => {
  // 844 px hoch, Morph über 0,8 Bildhöhen. Der alte Folger sprang hier um 0,57 in einem Bild.
  const { maxStep, m, moving } = drive(fling(8000), 0.8 * 844);
  assert.ok(maxStep < 0.1, `größter Schritt ${maxStep.toFixed(3)}`);
  assert.equal(m, 1);
  assert.equal(moving, false);
});

test('ein Sprung im Ziel (Anker, Ende des Scrollwegs) wird stetig eingeholt', () => {
  const s = { m: 0, v: 0 };
  let maxStep = 0;
  for (let f = 0; f < 60; f++) {
    const before = s.m;
    followStep(s, 1, 1 / 60);
    maxStep = Math.max(maxStep, s.m - before);
    // Kritisch gedämpft: nie über das Ziel hinaus.
    assert.ok(s.m <= 1);
  }
  assert.ok(maxStep < 0.12, `größter Schritt ${maxStep.toFixed(3)}`);
  assert.equal(s.m, 1);
});

test('langsames Scrollen folgt dicht und ohne Nachlaufen über das Ziel', () => {
  const s = { m: 0, v: 0 };
  for (let f = 0; f < 120; f++) {
    const goal = Math.min(0.5, f / 120);
    followStep(s, goal, 1 / 60);
    assert.ok(s.m <= goal + 1e-9, `m ${s.m} über ${goal}`);
  }
});

test('ein großer Zeitschritt nach einem verborgenen Tab bleibt stabil', () => {
  const s = { m: 0.2, v: 0 };
  followStep(s, 0.9, 5);
  assert.ok(s.m >= 0 && s.m <= 1 && Number.isFinite(s.v));
});
