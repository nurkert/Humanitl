// Tests der Choreografie der Bühne (src/scripts/stage/choreography.mjs, geometry.mjs, HUM-413):
// Oben steht immer derselbe Stapel (eigenes Fenster mit dem Ablauf, dahinter Modul 1), das
// Fenster mit dem Ablauf geht nach links aus dem Bild, kein Modul läuft als Kopie mit, ein
// gehendes Fenster tauscht nie die Ebene mit seinem Nachfolger, und über das Ringende springt
// nichts sichtbar.
//
//   node --test scripts/
import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  HERO_FAN,
  HERO_NEG,
  CAP_IN,
  CAP_OUT,
  LIVE_AT,
  REWIND,
  SHIFT,
  STAGE_NEG,
  STAGE_POS,
  exitShift,
  moduleShown,
  opacityOf,
  rewindSteps,
  ringAt,
  wrapRing,
} from '../src/scripts/stage/choreography.mjs';
import { heightOf, params, perspectiveOf, quadWith, slabT, slotPose, slotVec } from '../src/scripts/stage/geometry.mjs';

const N = 7;
const START = 0;
const Ws = 800;
// Die Bühne wie im Breitbild (layout.ts): frontal, Front tritt vor, links stehen gehende Fenster vorn.
const STAGE = params({ kappa: 0, yaw: 0, rho: 0, lift: 0.22, lead: 1, tilt: 2, beta: -22, phis: 26, r: 1.12, delta: 0.06 });
const CHOREO = {
  hero: params({ tilt: 4, phis: 18, r: 1.3, kappa: 0.25, delta: 0.36, yaw: -30, lift: 0.1 }),
  stage: STAGE,
  Ws,
  H: heightOf(Ws, 1.8935),
  stack: false,
  heroProfile: HERO_FAN,
  heroNeg: HERO_NEG,
  stagePos: STAGE_POS,
  stageNeg: STAGE_NEG,
};
const steps01 = Array.from({ length: 101 }, (_, i) => i / 100);

test('oben im Stapel steht immer Modul 1 direkt hinter dem Ablauf, egal welches Modul aktiv war', () => {
  for (let target = -9; target <= 16; target++) {
    const steps = rewindSteps(target, START, N);
    assert.ok(Math.abs(steps) <= N / 2, `steps ${steps} für ${target}`);
    const d = wrapRing(START - ringAt(target, target, steps, 0), N);
    assert.ok(Math.abs(d - 1) < 1e-9, `Ziel ${target}: Modul ${START} auf Platz ${d}`);
  }
});

test('auf der Bühne gilt die Lage des Rings selbst, und ohne Drehen steht Modul 1 im Fokus', () => {
  for (const target of [1, 3, 6, 7, 0]) {
    const steps = rewindSteps(target, START, N);
    for (const m of steps01.filter((v) => v >= REWIND[1])) assert.equal(ringAt(target, target, steps, m), target);
  }
  assert.equal(wrapRing(START - ringAt(START, START, rewindSteps(START, START, N), 1), N), 0);
});

test('beim Zurückscrollen dreht der Ring den kürzeren Weg, ohne Hin und Her, dann rücken die Module nach rechts', () => {
  // Zurückscrollen heißt fallendes m. In REWIND läuft die Lage stetig in Richtung der Schritte,
  // in SHIFT fällt sie um genau einen Platz.
  for (const target of [1, 2, 3, 4, 5, 6, 0]) {
    const steps = rewindSteps(target, START, N);
    assert.ok(Math.abs(steps) <= N / 2, `Ziel ${target}: ${steps} Schritte`);
    let last = null;
    for (const m of [...steps01].reverse()) {
      const at = ringAt(target, target, steps, m);
      if (last !== null && m >= REWIND[0]) assert.ok(Math.sign(at - last) * Math.sign(steps) >= 0, `Ziel ${target}: Hin und Her bei m=${m}`);
      if (last !== null && m < REWIND[0]) assert.ok(at <= last + 1e-12, `Ziel ${target}: vorwärts bei m=${m}`);
      last = at;
    }
  }
});

test('im Morph rückt Modul 1 stetig vom Platz hinter dem Ablauf in den Fokus', () => {
  const steps = rewindSteps(START, START, N);
  let last = 1;
  for (const m of steps01) {
    const d = wrapRing(START - ringAt(START, START, steps, m), N);
    assert.ok(d <= last + 1e-12 && d >= -1e-12 && d <= 1 + 1e-12, `d=${d} bei m=${m}`);
    last = d;
  }
  assert.ok(Math.abs(last) < 1e-9);
});

test('kein Modul läuft als Kopie hinter dem gehenden Fenster nach links', () => {
  // Das Modul auf Platz 0 oben (hinter dem Ablauf) und alle links davon bleiben im ganzen Morph
  // verborgen; Modul 1 und die rechts davon sind sichtbar.
  for (const target of [0, 3, 5]) {
    const steps = rewindSteps(target, START, N);
    for (const m of steps01.filter((v) => v < REWIND[0])) {
      const at = ringAt(target, target, steps, m);
      for (let i = 0; i < N; i++) {
        const dTop = wrapRing(i - ringAt(target, target, steps, 0), N);
        const d = wrapRing(i - at, N);
        const shown = moduleShown(d, m) && opacityOf(CHOREO, d, m) > 0;
        if (dTop <= 0) assert.equal(shown, false, `Ziel ${target}, Modul ${i} bei m=${m}`);
        if (dTop >= 1 && dTop <= 2) assert.equal(moduleShown(d, m), true, `Ziel ${target}, Modul ${i} bei m=${m}`);
      }
    }
  }
});

test('der Ausgang schiebt das Fenster ganz links aus dem Bild', () => {
  const view = { vw: 1440, vh: 900, Ws, H: CHOREO.H, P: perspectiveOf(Ws) };
  const G = { ...CHOREO.hero, S: 0.8, cx: 260, cy: 40 };
  const p = slotPose(G, 0, 1, Ws);
  const sh = exitShift(view, G, p, 48);
  const right = Math.max(...quadWith(view, G, { ...p, x: p.x - sh }).map((c) => c[0]));
  assert.ok(right <= -47, `rechte Kante bei ${right.toFixed(1)} px`);
  assert.ok(sh > 0);
});

/** Tiefe am Platz d auf der Bühne (Rig-Einheiten). */
const zAt = (d) => slotVec(STAGE, d, Ws)[2];
const opAt = (d) => opacityOf(CHOREO, d, 1);

test('rechts vom Fokus liegt jedes Fenster tiefer als sein linker Nachbar', () => {
  for (let d = 0; d < 3; d += 0.05) assert.ok(zAt(d + 0.05) < zAt(d), `z steigt bei d=${d.toFixed(2)}`);
});

test('ein gehendes Fenster bleibt auf seinem ganzen sichtbaren Weg vor dem kommenden', () => {
  // Abstand der Ebenen, unter dem die Scheiben (Dicke T) sich durchdrängen oder der Browser die
  // Reihenfolge wechseln könnte.
  const margin = slabT(Ws) + 6;
  for (const t of steps01) {
    // Weiter: das Fenster im Fokus geht nach links (d = -t), das nächste kommt von rechts.
    const leaving = -t;
    const coming = 1 - t;
    if (opAt(leaving) > 0.02 && opAt(coming) > 0.02)
      assert.ok(zAt(leaving) - zAt(coming) >= margin, `weiter t=${t}: Abstand ${(zAt(leaving) - zAt(coming)).toFixed(1)}`);
    // Zurück: das Fenster von links kommt in den Fokus (d = t - 1), das bisherige geht nach rechts.
    const fromLeft = t - 1;
    const toRight = t;
    if (opAt(fromLeft) > 0.02 && opAt(toRight) > 0.02)
      assert.ok(zAt(fromLeft) - zAt(toRight) >= margin, `zurück t=${t}: Abstand ${(zAt(fromLeft) - zAt(toRight)).toFixed(1)}`);
  }
});

test('ein gehendes Fenster ist nach einem halben Platz ausgeblendet', () => {
  assert.ok(opAt(-0.5) <= 0.001);
  assert.ok(opAt(-0.3) < 0.6);
  assert.equal(opAt(0), 1);
});

test('über das Ringende springt nichts sichtbar, in jedem Stand des Morphs', () => {
  for (const m of steps01) {
    for (const d of [-3.5, -3.2, -3, 3, 3.2, 3.49]) assert.equal(opacityOf(CHOREO, d, m), 0, `d=${d}, m=${m}`);
  }
});

test('das Zurückdrehen endet, bevor die Beschriftung ausblendet; alles vor der Bühne', () => {
  assert.ok(SHIFT[1] <= REWIND[0], 'SHIFT und REWIND überlappen');
  assert.ok(REWIND[1] < CAP_OUT, 'Beschriftung zeigte ein verlassenes Modul');
  assert.ok(CAP_OUT < CAP_IN && CAP_IN < LIVE_AT);
});

test('ein Zug oder eine Drehung, die beim Verlassen der Bühne noch läuft, verschiebt den Stapel oben nie', () => {
  // pos gleitet noch (mitten in einer Drehung, einem Zug, einem Rad), target ist das Ziel.
  for (const target of [0, 1, 4, 6, 9]) {
    const steps = rewindSteps(target, START, N);
    for (const pos of [target - 0.49, target - 0.2, target, target + 0.3, target + 0.49]) {
      for (const m of steps01.filter((v) => v <= REWIND[0])) {
        // Unterhalb von REWIND zählt allein das Ziel: dieselbe Lage wie ohne laufende Bewegung.
        assert.equal(ringAt(pos, target, steps, m), ringAt(target, target, steps, m), `pos ${pos}, m=${m}`);
      }
      // Oben steht Modul 1 hinter dem Ablauf, also springt das Zurücksetzen nicht.
      assert.ok(Math.abs(wrapRing(START - ringAt(pos, target, steps, 0), N) - 1) < 1e-9);
    }
    // Und auf der Bühne folgt der Ring der gleitenden Lage selbst.
    assert.equal(ringAt(target + 0.3, target, steps, 1), target + 0.3);
  }
});

test('nach einem Wurf, der beim Verlassen der Bühne noch lief, dreht der Ring vom sichtbaren Stand aus kurz zurück', () => {
  // Wurf von 1,2 Richtung 4, angehalten bei der Lage, die man sieht: pos = target = 1,2.
  for (const pos of [1.2, 3.7, -2.4, 6.49, 0.3]) {
    const steps = rewindSteps(pos, START, N);
    assert.ok(Math.abs(steps) <= N / 2 + 1e-9, `pos ${pos}: ${steps} Schritte`);
    // Oben steht Modul 1 genau hinter dem Ablauf, ohne Rest eines halben Platzes.
    assert.ok(Math.abs(wrapRing(START - ringAt(pos, pos, steps, 0), N) - 1) < 1e-9, `pos ${pos}`);
  }
  assert.ok(Math.abs(rewindSteps(1.2, START, N) + 1.2) < 1e-9);
});
