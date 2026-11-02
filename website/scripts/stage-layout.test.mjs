// Tests der Lage der Bühne (src/scripts/stage/layout-solve.mjs, HUM-413): Kein sichtbares Fenster
// kommt der Beschriftung näher als der Abstand, in keiner Größe; was nachgibt, ist erst der
// Abstand der Fenster, dann die Steigung, und erst zuletzt die Größe der Front.
//
//   node --test scripts/
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { STAGE_NEG, STAGE_POS } from '../src/scripts/stage/choreography.mjs';
import { heightOf, params, perspectiveOf, stageRect } from '../src/scripts/stage/geometry.mjs';
import { captionGap, solveStage, visibleDs } from '../src/scripts/stage/layout-solve.mjs';

const ASPECT = 1.8935;
const STAGE = params({ kappa: 0, yaw: 0, rho: 0, lift: 0.22, lead: 1, tilt: 2, beta: -18, phis: 30, r: 1.12, delta: 0.06 });

/** Eine Beschriftung rechts unten wie im Breitbild: Kopf mit Zählwerk und Textfläche. */
function sideCaption(vw, vh, height) {
  const r = vw * 0.96;
  const l = r - vw * 0.3;
  const b = vh * 0.94;
  return [
    { l, t: b - height, r: l + 220, b: b - height + 90 },
    { l, t: b - height + 100, r, b },
  ];
}

function input(vw, vh, caps, extra = {}) {
  return {
    vw,
    vh,
    aspect: ASPECT,
    Ws0: Math.min(vw * 0.58, vh * 1.05),
    minWs: Math.min(vw * 0.58, vh * 1.05) * 0.4,
    stage: STAGE,
    phisMin: 18,
    betaMax: -26,
    edge: 0.035,
    navBottom: 72,
    caps,
    below: null,
    ctl: { h: 50, w: 384, gap: 24 },
    bottom: 14,
    gutter: Math.max(24, vw * 0.018),
    stagePos: STAGE_POS,
    stageNeg: STAGE_NEG,
    side: Math.max(10, vw * 0.02),
    ...extra,
  };
}

const viewOf = (I, Ws) => ({ vw: I.vw, vh: I.vh, Ws, H: heightOf(Ws, ASPECT), P: perspectiveOf(Ws) });

test('captionGap misst eine Überdeckung negativ und einen freien Kasten positiv', () => {
  const vw = 1440;
  const vh = 900;
  const Ws = 640;
  const G = { ...STAGE, cx: -200, cy: 0 };
  const view = viewOf({ vw, vh }, Ws);
  const front = stageRect(view, G, 0);
  const over = { l: front.r - 40, t: front.t + 20, r: front.r + 200, b: front.t + 120 };
  const free = { l: front.r + 300, t: front.t, r: front.r + 400, b: front.b };
  assert.ok(captionGap(view, G, [0], [over]) < 0);
  assert.ok(captionGap(view, G, [0], [free]) > 0);
});

for (const [vw, vh, capH] of [
  [1280, 720, 420],
  [1280, 800, 460],
  [1366, 768, 430],
  [1440, 900, 470],
  [1536, 864, 450],
  [1770, 870, 470],
  [1920, 1080, 520],
  [2560, 1080, 520],
  [2560, 1440, 600],
]) {
  test(`${vw}x${vh}: kein sichtbares Fenster berührt die Beschriftung, die Steuerung passt`, () => {
    const I = input(vw, vh, sideCaption(vw, vh, capH));
    const out = solveStage(I);
    assert.ok(out.ok, 'keine Lösung');
    const view = viewOf(I, out.Ws);
    const gap = captionGap(view, out.stage, visibleDs(STAGE_POS, STAGE_NEG), I.caps);
    assert.ok(gap >= I.gutter, `Abstand ${gap.toFixed(1)} < ${I.gutter}`);
    // Auch zwischen den Proben, mitten in einer Drehung (Schritte von 0,02 Plätzen).
    // Jeder Zwischenstand einzeln, ohne Hülle (eine Probe je Aufruf).
    for (let d = -0.5; d <= 2.55; d += 0.02) {
      const one = captionGap(view, out.stage, [d], I.caps);
      assert.ok(one >= I.gutter - 0.5, `d=${d.toFixed(2)}: ${one.toFixed(1)} < ${I.gutter}`);
    }
    assert.ok(out.front.t >= I.navBottom + 15.5, 'Front unter der Navigation');
    assert.ok(out.ctl.y + I.ctl.h <= vh - I.bottom + 0.5, 'Steuerung im Bild');
  });
}

test('vor der Front gibt erst der Abstand der Fenster nach, dann die Steigung', () => {
  // Eine Beschriftung, die erst bei engerem Abstand frei wird: die Front behält ihre Größe.
  const vw = 1440;
  const vh = 900;
  const I = input(vw, vh, sideCaption(vw, vh, 300), { Ws0: 600, minWs: 240 });
  const free = solveStage({ ...I, caps: [] });
  assert.equal(free.Ws, 600);
  assert.equal(free.stage.phis, STAGE.phis);
  assert.equal(free.stage.beta, STAGE.beta);
  const out = solveStage(I);
  assert.ok(out.ok);
  if (out.Ws < 600) {
    // Nur wenn kein Abstand und keine Steigung half, darf die Front kleiner sein.
    assert.equal(out.stage.phis <= I.phisMin + 1e-6 || out.stage.beta <= I.betaMax + 1e-6, true);
  }
});

test('Hochformat: die Beschriftung beginnt unter der Front, nichts reicht in sie hinein', () => {
  for (const [vw, vh] of [
    [320, 640],
    [360, 740],
    [390, 844],
    [430, 932],
    [768, 1024],
  ]) {
    const I = input(vw, vh, [], {
      Ws0: vw * 0.92,
      minWs: vw * 0.3,
      edge: null,
      ctl: null,
      side: 10,
      stage: params({ ...STAGE, beta: -6, phis: 17, r: 2.4, lift: 0.08 }),
      phisMin: 13,
      betaMax: -9,
      below: { need: Math.min(330, vh * 0.42), gap: 18 },
      navBottom: 64,
    });
    const out = solveStage(I);
    assert.ok(out.ok, `${vw}x${vh}: keine Lösung`);
    const view = viewOf(I, out.Ws);
    const below = [{ l: -1e5, t: out.capTop, r: 1e5, b: 1e5 }];
    assert.ok(captionGap(view, out.stage, visibleDs(STAGE_POS, STAGE_NEG), below) >= 17.5, `${vw}x${vh}`);
    assert.ok(out.front.l >= 9.5 && out.front.r <= vw - 9.5, `${vw}x${vh}: Front im Bild`);
    assert.ok(out.capTop + I.below.need <= vh - I.bottom + 0.5, `${vw}x${vh}: Beschriftung im Bild`);
  }
});
