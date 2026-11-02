// Tests der Klassen und der Zusammenstellung der Bühne (src/scripts/stage/classes.mjs,
// assemble.mjs, HUM-413): Grenzen der Klassen, stimmige Vorgaben je Klasse, nichts Sichtbares
// über das Ringende, und das Fenster mit dem Ablauf gibt es nur im Stapel.
//
//   node --test scripts/
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { assemble } from '../src/scripts/stage/assemble.mjs';
import { HERO_NEG, REWIND, STAGE_NEG, opacityOf } from '../src/scripts/stage/choreography.mjs';
import { BELOW, CTL_UNDER, EDGE, FLAT, SIDE, SPEC, classify } from '../src/scripts/stage/classes.mjs';
import { heightOf, params, perspectiveOf, quadWith } from '../src/scripts/stage/geometry.mjs';

test('classify ordnet die üblichen Bildausschnitte ihren Klassen zu', () => {
  const cases = [
    [2560, 1080, 'A'],
    [1770, 870, 'A'],
    [1920, 1080, 'B'],
    [1366, 768, 'B'],
    [1440, 900, 'C'],
    [1280, 800, 'C'],
    [1024, 768, 'D'],
    [768, 1024, 'E'],
    [390, 844, 'F'],
    [320, 640, 'F'],
    [844, 390, 'G'],
  ];
  for (const [w, h, cls] of cases) assert.equal(classify(w, h), cls, `${w}x${h}`);
});

test('jede Klasse hat stimmige Vorgaben', () => {
  for (const [cls, spec] of Object.entries(SPEC)) {
    const stage = params({ ...FLAT, ...spec.stage });
    assert.ok(spec.phisMin <= stage.phis, `${cls}: phisMin über phis`);
    assert.ok(spec.betaMax <= stage.beta, `${cls}: betaMax flacher als beta`);
    assert.ok(spec.ws(1440, 900, 1.89) > 0 && spec.morph > 0, cls);
    assert.ok(cls in EDGE, cls);
    // Eine Klasse steht entweder seitlich oder unter der Bühne beschriftet, nie beides.
    assert.ok(!(SIDE.has(cls) && BELOW.has(cls)), cls);
    // Die Steuerung unter der Front gibt es nur dort, wo die Beschriftung nicht darunter steht.
    if (CTL_UNDER.has(cls)) assert.ok(!BELOW.has(cls), cls);
  }
});

test('über das Ringende ist in keiner Klasse etwas zu sehen, auch nicht im Fächer von A', () => {
  // A zeigt im Fächer vier Scheiben (d = 3 halb); erst ab 3,4 ist alles aus, vor dem Sprung bei 3,5.
  for (const [cls, spec] of Object.entries(SPEC)) {
    const c = { heroProfile: spec.heroProfile, heroNeg: HERO_NEG, stagePos: spec.stagePos, stageNeg: STAGE_NEG };
    for (let m = 0; m <= 1; m += 0.05)
      for (const d of [-3.5, -3.45, -3.4, 3.4, 3.45, 3.49]) assert.equal(opacityOf(c, d, m), 0, `${cls} d=${d} m=${m}`);
  }
});

const Ws = 800;
const VIEW = { vw: 1440, vh: 900, Ws, H: heightOf(Ws, 1.8935), P: perspectiveOf(Ws) };
const C = {
  hero: params({ ...SPEC.C.hero, S: 0.8, cx: 280 }),
  stage: params({ ...FLAT, ...SPEC.C.stage, cx: -260 }),
  Ws,
  H: VIEW.H,
  stack: false,
  heroProfile: SPEC.C.heroProfile,
  heroNeg: HERO_NEG,
  stagePos: SPEC.C.stagePos,
  stageNeg: STAGE_NEG,
};

test('das Fenster mit dem Ablauf steht oben vorn und ist ab REWIND links aus dem Bild verschwunden', () => {
  const ring = { n: 7, pos: 0, target: 0, steps: 0, lead: true, reduced: false };
  const top = assemble(C, C.hero, VIEW, { ...ring, m: 0 });
  assert.equal(top.ds.length, 8);
  assert.equal(top.ds[7], -0);
  assert.equal(top.ops[7], 1);
  // Modul 1 steht direkt dahinter, das Modul auf Platz 0 ist verborgen.
  assert.ok(Math.abs(top.ds[0] - 1) < 1e-9 && top.ops[0] > 0.9);
  assert.equal(top.ops[top.ds.findIndex((d, i) => i < 7 && Math.abs(d) < 1e-9)], 0);
  for (const m of [0.3, 0.45, 0.49]) {
    const mid = assemble(C, C.hero, VIEW, { ...ring, m });
    const right = Math.max(...quadWith(VIEW, C.hero, mid.poses[7]).map((q) => q[0]));
    if (m >= 0.45) assert.ok(right < 0, `m=${m}: rechte Kante ${right.toFixed(0)}`);
  }
  const stage = assemble(C, C.stage, VIEW, { ...ring, m: REWIND[0] });
  assert.equal(stage.ops[7], 0);
  assert.equal(stage.exiting, false);
});

test('ohne Fenster mit dem Ablauf gibt es genau sieben Einträge', () => {
  const r = assemble(C, C.stage, VIEW, { n: 7, pos: 2, target: 2, steps: 0, m: 1, lead: false, reduced: false });
  assert.equal(r.ds.length, 7);
  assert.equal(r.ds[2], 0);
});
