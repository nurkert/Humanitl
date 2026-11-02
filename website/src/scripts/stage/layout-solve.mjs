// Wo die Bühne steht und wie groß ihr Fenster im Fokus ist (HUM-413). Rein und ohne DOM: die
// Maße der Beschriftung, der Navigation und der Steuerung kommen gemessen herein, heraus kommen
// Fensterbreite, Parametersatz und die Lage von Steuerung und Beschriftung.
//
// Harte Regeln, in jeder Breite und Höhe:
// - Kein sichtbares Fenster (Deckkraft ab 5 %, auch auf halbem Weg einer Drehung und das
//   gehende links) kommt der Beschriftung näher als gutter. Gemessen wird das projizierte
//   Viereck des Fensters gegen jeden Kasten der Beschriftung.
// - Die Front steht unter der Navigation, ihre Steuerung passt darunter ins Bild.
// - Steht die Beschriftung unter der Bühne (Hochformat), beginnt sie unter der Front und passt
//   samt Steuerung und sicherem Rand ins Bild.
// Was nachgibt, in dieser Reihenfolge: erst der Abstand der Fenster auf dem Bogen, dann die
// Steigung, dann die Höhe der Bühne, und erst zuletzt die Größe des Fensters im Fokus (Inhaber,
// 2026-11-02). Ohne Konflikt steht die Gruppe aus Front, Nachbar und Steuerung senkrecht mittig
// zwischen Navigation und Boden.

import { profile } from "./math.mjs";
import { boxGap, heightOf, perspectiveOf, stageRect } from "./geometry.mjs";

/** @typedef {import("./geometry.mjs").Params} Params */
/** @typedef {import("./geometry.mjs").Box} Box */

/**
 * @typedef {object} SolveInput
 * @property {number} vw
 * @property {number} vh
 * @property {number} aspect
 * @property {number} Ws0 gewünschte Fensterbreite
 * @property {number} minWs kleinste Fensterbreite, die noch versucht wird
 * @property {Params} stage bevorzugter Parametersatz der Bühne
 * @property {number} phisMin kleinster Winkelabstand
 * @property {number} betaMax steilste Steigung (negativ)
 * @property {number | null} edge linker Rand der Front als Anteil der Breite, null: mittig
 * @property {number} navBottom
 * @property {Box[]} caps Kästen der Beschriftung (seitlich), leer bei below
 * @property {{ need: number, gap: number } | null} below Beschriftung unter der Front: ihre
 *   Höhe samt Steuerung und Boden, und der Abstand zur Front
 * @property {{ h: number, w: number, gap: number } | null} ctl Steuerung mittig unter der Front
 * @property {number} bottom freier Rand unten (sicherer Bereich plus Luft)
 * @property {number} gutter
 * @property {import("./math.mjs").Profile} stagePos
 * @property {import("./math.mjs").Profile} stageNeg
 * @property {number} [side] Rand links und rechts der Front, wenn sie mittig steht
 */

/**
 * @typedef {object} Solved
 * @property {number} Ws
 * @property {Params} stage
 * @property {Box} front
 * @property {{ x: number, y: number, w: number } | null} ctl
 * @property {number | null} capTop
 * @property {number} gap kleinster Abstand eines Fensters zur Beschriftung
 * @property {boolean} ok alle Regeln erfüllt
 */

/**
 * Plätze, an denen ein Fenster im Lauf einer Drehung steht und sichtbar sein kann: der ganze
 * Weg in Schritten von 0,1, auch das gehende Fenster links. Zwischen zwei Proben bewegt sich
 * ein Fenster auf der Bühne um höchstens einige Pixel; nur ganze Plätze zu prüfen ließ es mitten
 * in einer Drehung bis auf 15 px an die Beschriftung (Review HUM-413).
 */
export const PROBE_DS = Array.from({ length: 32 }, (_, i) => Math.round((i / 10 - 0.5) * 10) / 10);

/**
 * Die Probeplätze für die Profile der Bühne: jeder sichtbare Platz und sein Nachbar, damit die
 * Hülle in captionGap auch das letzte Stück vor dem Ausblenden deckt.
 * @param {import("./math.mjs").Profile} pos @param {import("./math.mjs").Profile} neg
 */
export const visibleDs = (pos, neg) => {
  const seen = (/** @type {number} */ d) => profile(d >= 0 ? pos : neg, Math.abs(d)) >= 0.05;
  return PROBE_DS.filter((d) => seen(d) || seen(d - 0.1) || seen(d + 0.1));
};

/**
 * Kleinster Abstand der Fenster an den Plätzen ds zu den Kästen caps; negativ heißt Überdeckung.
 * Zwischen zwei benachbarten Proben zählt die Hülle beider Kästen: Ein Fenster auf halbem Weg
 * liegt in ihr, so ist der Abstand auch mitten in einer Drehung gesichert, nicht nur an den
 * Proben (Review HUM-413). ds muss aufsteigend sortiert sein.
 * @param {import("./geometry.mjs").View} view @param {Params} G @param {number[]} ds @param {Box[]} caps
 */
export function captionGap(view, G, ds, caps) {
  if (!caps.length) return Infinity;
  let gap = Infinity;
  /** @type {Box | null} */
  let prev = null;
  let prevD = -Infinity;
  for (const d of ds) {
    const r = stageRect(view, G, d);
    // Nur benachbarte Proben bilden eine Hülle; eine Lücke in ds trennt zwei Wege.
    const hull =
      prev && d - prevD <= 0.11
        ? { l: Math.min(prev.l, r.l), t: Math.min(prev.t, r.t), r: Math.max(prev.r, r.r), b: Math.max(prev.b, r.b) }
        : r;
    for (const c of caps) gap = Math.min(gap, boxGap(hull, c));
    prev = r;
    prevD = d;
  }
  return gap;
}

/**
 * Rückt die Bühne waagrecht an ihren Rand und senkrecht mittig in den freien Raum. Die
 * Projektion ist in cx und cy fast linear; drei Schritte reichen auf Bruchteile eines Pixels.
 * @param {SolveInput} I @param {import("./geometry.mjs").View} view @param {Params} G
 * @param {number} lift Verschiebung nach oben in px (negativ: nach oben)
 */
export function place(I, view, G, lift) {
  for (let k = 0; k < 3; k++) {
    const f = stageRect(view, G, 0);
    G.cx += I.edge === null ? (I.vw - f.l - f.r) / 2 : Math.round(I.edge * I.vw) - f.l;
    const n1 = stageRect(view, G, 1);
    const top = Math.min(f.t, n1.t + 0.5 * (f.t - n1.t));
    let bottom = f.b;
    if (I.ctl) bottom += I.ctl.gap + I.ctl.h;
    if (I.below) bottom += I.below.gap + I.below.need;
    const roomTop = I.navBottom + 16;
    const roomBottom = I.vh - I.bottom;
    let dy = (roomTop + roomBottom) / 2 - (top + bottom) / 2 + lift;
    // Nie über die Navigation: dann lieber tiefer.
    if (f.t + dy < roomTop) dy = roomTop - f.t;
    G.cy += dy;
  }
}

/**
 * Unterkante des tiefsten sichtbaren Fensters an den Plätzen ds: Darunter beginnt eine
 * Beschriftung im Hochformat, auch ein gehendes Fenster, das vortritt, reicht nie hinein.
 * @param {import("./geometry.mjs").View} view @param {Params} G @param {number[]} ds
 */
export function lowestBottom(view, G, ds) {
  return Math.max(...ds.map((d) => stageRect(view, G, d).b));
}

/**
 * Prüft alle Regeln für G; liefert den Abstand zur Beschriftung oder null, wenn eine Regel
 * verletzt ist.
 * @param {SolveInput} I @param {import("./geometry.mjs").View} view @param {Params} G
 * @param {number[]} ds
 */
export function check(I, view, G, ds) {
  const f = stageRect(view, G, 0);
  const side = I.side ?? 10;
  if (f.t < I.navBottom + 15.5) return null;
  if (f.l < side - 0.5) return null;
  if (I.edge === null && f.r > I.vw - side + 0.5) return null;
  let bottom = f.b;
  if (I.ctl) {
    const cw = Math.min(I.ctl.w, f.r - f.l);
    const cx = (f.l + f.r) / 2;
    const ct = f.b + I.ctl.gap;
    const box = { l: cx - cw / 2, t: ct, r: cx + cw / 2, b: ct + I.ctl.h };
    bottom = box.b;
    // Die Steuerung selbst hält Abstand zur Beschriftung.
    for (const c of I.caps) if (boxGap(box, c) < I.gutter * 0.5) return null;
  }
  if (I.below) bottom = lowestBottom(view, G, ds) + I.below.gap + I.below.need;
  if (bottom > I.vh - I.bottom + 0.5) return null;
  const gapCaps = captionGap(view, G, ds, I.caps);
  if (gapCaps < I.gutter) return null;
  return I.below ? Math.min(gapCaps, I.below.gap) : gapCaps;
}

/**
 * Die beste Lage für eine feste Fensterbreite, in der Reihenfolge der Vorlieben: erst mittig,
 * dann mit weniger Abstand der Fenster, dann steiler, dann höher. null, wenn keine passt.
 * @param {SolveInput} I @param {number} Ws @param {number[]} ds
 * @returns {Solved | null}
 */
function fit(I, Ws, ds) {
  const view = { vw: I.vw, vh: I.vh, Ws, H: heightOf(Ws, I.aspect), P: perspectiveOf(Ws) };
  for (const lift of [0, -0.05, -0.1]) {
    for (let phis = I.stage.phis; phis >= I.phisMin - 1e-6; phis -= 3) {
      for (let beta = I.stage.beta; beta >= I.betaMax - 1e-6; beta -= 3) {
        const G = { ...I.stage, phis, beta, cx: 0, cy: 0 };
        place(I, view, G, lift * I.vh);
        const gap = check(I, view, G, ds);
        if (gap !== null) return result(I, view, G, ds, gap, true);
      }
    }
  }
  return null;
}

/**
 * Sucht die größte Front, die alle Regeln erfüllt. Ob eine Breite passt, wächst nicht mit ihr;
 * eine Halbierung des Bereichs findet die größte auf 1 % genau in wenigen Schritten.
 * @param {SolveInput} I
 * @returns {Solved}
 */
export function solveStage(I) {
  const ds = visibleDs(I.stagePos, I.stageNeg);
  const top = fit(I, I.Ws0, ds);
  if (top) return top;
  let best = fit(I, I.minWs, ds);
  if (!best) {
    // Nichts passt: die kleinste Bühne mit ihrer Lage, als Fehlschlag gekennzeichnet.
    const view = { vw: I.vw, vh: I.vh, Ws: I.minWs, H: heightOf(I.minWs, I.aspect), P: perspectiveOf(I.minWs) };
    const G = { ...I.stage, cx: 0, cy: 0 };
    place(I, view, G, 0);
    return result(I, view, G, ds, -1, false);
  }
  let lo = I.minWs;
  let hi = I.Ws0;
  while (hi - lo > I.Ws0 * 0.01) {
    const mid = (lo + hi) / 2;
    const got = fit(I, mid, ds);
    if (got) {
      best = got;
      lo = mid;
    } else hi = mid;
  }
  return best;
}

/**
 * @param {SolveInput} I @param {import("./geometry.mjs").View} view @param {Params} G
 * @param {number[]} ds @param {number} gap @param {boolean} ok
 * @returns {Solved}
 */
function result(I, view, G, ds, gap, ok) {
  const front = stageRect(view, G, 0);
  const ctl = I.ctl
    ? {
        x: Math.round((front.l + front.r) / 2),
        y: Math.round(front.b + I.ctl.gap),
        w: Math.round(Math.min(I.ctl.w, front.r - front.l)),
      }
    : null;
  const capTop = I.below ? Math.round(lowestBottom(view, G, ds) + I.below.gap) : null;
  return { Ws: view.Ws, stage: G, front, ctl, capTop, gap, ok };
}
