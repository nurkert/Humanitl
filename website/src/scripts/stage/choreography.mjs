// Die Choreografie der Bühne (HUM-334, HUM-413): wie Fenster vom Stapel der Startseite auf
// den Bogen des Karussells wandern, wer dabei vorn liegt und wann wer sichtbar ist. Rein und
// ohne DOM, damit die Tests Posen, Reihenfolge und Deckkraft nachrechnen können.
//
// Ablauf seit HUM-413: Im Stapel steht vorn ein eigenes Fenster mit dem aufgenommenen Ablauf,
// das zu keinem Modul gehört; dahinter stehen die Module ab start (Modul 1). Beim Scrollen
// verlässt das vordere Fenster das Bild sauber nach links, die Module rücken je einen Platz
// nach und kommen auf der Bühne an, mit Modul start im Fokus. Beim Zurückscrollen kommt das
// Fenster von links wieder herein. Hat jemand das Karussell gedreht, dreht der Ring beim
// Zurückscrollen erst auf dem kürzeren Weg zurück, bis wieder Modul start vorn steht, so dass
// oben immer derselbe Stapel steht und nie ein veraltetes Modul.

import {
  clamp,
  easeInOutCubic,
  easeInOutSine,
  lerp,
  map,
  profile,
  smootherstep,
  smoothstep,
} from "./math.mjs";
import { project, slotPose, toWorld, winPoint } from "./geometry.mjs";

/** @typedef {import("./geometry.mjs").Params} Params */
/** @typedef {import("./geometry.mjs").Pose} Pose */
/** @typedef {import("./math.mjs").Profile} Profile */

/**
 * Was die Choreografie vom Layout braucht.
 * @typedef {object} Choreo
 * @property {Params} hero
 * @property {Params} stage
 * @property {number} Ws
 * @property {number} H
 * @property {boolean} stack Telefon hoch: Kartenstapel statt Fächer
 * @property {Profile} heroProfile
 * @property {Profile} heroNeg
 * @property {Profile} stagePos
 * @property {Profile} stageNeg
 */

export const HERO_FAN = /** @type {Profile} */ ([
  [1, 1],
  [2, 0.85],
  [2.6, 0],
]);
// Links der Front liegt im Fächer nichts: dort warten die Fenster unsichtbar auf den Morph.
export const HERO_NEG = /** @type {Profile} */ ([
  [0, 1],
  [0.6, 0],
]);
// Rechts der Front stehen auf der Bühne zwei Nachbarn, der dritte blendet aus.
export const STAGE_POS = /** @type {Profile} */ ([
  [2, 1],
  [2.6, 0],
]);
// Links der Front steht auf der Bühne nichts (HUM-389). Ein gehendes Fenster blendet auf dem
// ersten halben Platz aus, auf seinem eigenen Weg vor allen anderen (HUM-413).
export const STAGE_NEG = /** @type {Profile} */ ([
  [0, 1],
  [0.1, 0.94],
  [0.5, 0],
]);

// Bänder des Morphs, in m. Das Fenster mit dem Ablauf geht in EXIT nach links aus dem Bild;
// der Ring rückt in SHIFT einen Platz weiter. In REWIND dreht er beim Zurückscrollen, bis der
// Anfang wieder vorn steht. REWIND endet, bevor die Beschriftung beim Zurückscrollen
// ausblendet (CAP_OUT): sie zeigt nie ein Modul, das der Ring schon verlassen hat. Auf der
// Bühne (ab LIVE_AT) gilt immer die Lage des Rings selbst.
export const EXIT = /** @type {[number, number]} */ ([0.03, 0.45]);
export const SHIFT = /** @type {[number, number]} */ ([0.04, 0.5]);
export const REWIND = /** @type {[number, number]} */ ([0.5, 0.72]);
// Die Beschriftung erscheint als Ganzes ab CAP_IN und geht unter CAP_OUT; dazwischen bleibt sie,
// wie sie ist (keine halben Stände beim Anhalten).
export const CAP_OUT = 0.74;
export const CAP_IN = 0.8;
export const LIVE_AT = 0.85;

/**
 * Abstand auf dem Ring, gefaltet in [-n/2, n/2): jedes Fenster hat links und rechts Nachbarn.
 * @param {number} d @param {number} n
 */
export const wrapRing = (d, n) => ((((d + n / 2) % n) + n) % n) - n / 2;

/** @param {number} v @param {number} n */
export const modRing = (v, n) => ((v % n) + n) % n;

/**
 * Wie viele Schritte der Ring beim Zurückscrollen dreht, bis Modul start wieder vorn steht:
 * der kürzere Weg von der Lage pos aus, wie sie beim Verlassen der Bühne zu sehen ist, höchstens
 * eine halbe Runde (positiv vorwärts, auch Bruchteile, wenn der Ring mitten in einer Drehung
 * angehalten hat). Ohne Drehen auf der Bühne ist es keiner. Beide Richtungen sind sauber: vorwärts bleibt ein gehendes Fenster vor allen
 * anderen und blendet links aus, rückwärts kommt eines von links vor allen anderen herein, wie
 * mit dem Pfeil zurück. Ein Wurf nach oben dreht so nie fast eine ganze Runde.
 * @param {number} pos Lage des Rings beim Verlassen der Bühne
 * @param {number} start Modul im Fokus, wenn die Bühne ankommt
 * @param {number} n
 */
export const rewindSteps = (pos, start, n) => wrapRing(start - pos, n);

/**
 * Die Lage des Rings, nach der die Module stehen: auf der Bühne (m ab REWIND[1]) die Lage pos
 * selbst, oben im Stapel target + steps - 1, also Modul start auf Platz 1 hinter dem Fenster mit
 * dem Ablauf. Dazwischen erst das Weiterdrehen in REWIND, dann der eine Schritt in SHIFT, mit
 * dem die Module auf den Platz des gehenden Fensters rücken.
 *
 * Unterhalb der Bühne geht die gleitende Lage pos in REWIND auf ihr ganzzahliges Ziel target
 * über: Eine Drehung, ein Zug oder ein Rad, das beim Verlassen der Bühne noch läuft, verschiebt
 * den Stapel oben also nie, und das Zurücksetzen oben springt nicht.
 * @param {number} pos @param {number} target @param {number} steps @param {number} m
 */
export function ringAt(pos, target, steps, m) {
  const kV = smoothstep(map(m, SHIFT[0], SHIFT[1]));
  const kR = smoothstep(map(m, REWIND[0], REWIND[1]));
  return lerp(target, pos, kR) + steps * (1 - kR) - (1 - kV);
}

/**
 * Ob ein Modul am Platz d im Morph zu sehen sein darf. Platz 0 gehört oben dem Fenster mit dem
 * Ablauf: Das Modul, das dort hinter ihm stünde, und alle links davon bleiben verborgen, bis der
 * Ring in SHIFT einen Platz weitergerückt ist; sonst liefe eine Kopie hinter dem gehenden Fenster
 * mit nach links. Die Grenze wandert mit dem Ring, auf der Bühne liegt sie bei d = -0,5, wo ein
 * gehendes Fenster ohnehin ausgeblendet ist (STAGE_NEG).
 * @param {number} d @param {number} m
 */
export const moduleShown = (d, m) => d >= 0.5 - smoothstep(map(m, SHIFT[0], SHIFT[1]));

/** Wie weit das Fenster mit dem Ablauf aus dem Bild ist, 0 bis 1. @param {number} m */
export const exitAmount = (m) => smoothstep(map(m, EXIT[0], EXIT[1]));

/**
 * Verschiebung in Rig-Einheiten entlang x, nach der die rechte Kante des Fensters mit Pose p
 * links aus dem Bild ist (mit Rand margin in px). Ein paar Sekantenschritte genügen, weil die
 * Verschiebung die Tiefe kaum ändert.
 * @param {import("./geometry.mjs").View} view @param {Params} G @param {Pose} p
 * @param {number} [margin]
 */
export function exitShift(view, G, p, margin = 48) {
  const right = (/** @type {number} */ s) =>
    Math.max(
      ...[-1, 1].flatMap((sy) =>
        [-1, 1].map(
          (sx) =>
            project(
              view,
              toWorld(G, winPoint({ ...p, x: p.x - s }, (sx * view.Ws) / 2, (sy * view.H) / 2, 0, view.H)),
            )[0],
        ),
      ),
    );
  let s = 0;
  for (let i = 0; i < 4; i++) {
    const r = right(s);
    if (r <= -margin + 0.5) break;
    // Pixel je Rig-Einheit an der Front, aus einem kleinen Schritt gemessen.
    const k = Math.max(0.05, (right(s) - right(s + 10)) / 10);
    s += (r + margin) / k;
  }
  return s;
}

/**
 * Der Parametersatz für den Morph m, je Gruppe mit eigener Kurve.
 * @param {Params} h @param {Params} s @param {number} mm
 * @returns {Params}
 */
export function mixG(h, s, mm) {
  // S und die Lage sprechen sofort an, sobald der Text zu gehen beginnt.
  const eS = easeInOutSine(mm);
  const eArc = smootherstep(map(mm, 0.05, 0.9));
  const eC = easeInOutSine(map(mm, 0.04, 0.85));
  const eT = easeInOutSine(map(mm, 0, 0.85));
  const eB = smoothstep(map(mm, 0.35, 1));
  return {
    S: lerp(h.S, s.S, eS),
    cx: lerp(h.cx, s.cx, eC),
    cy: lerp(h.cy, s.cy, eC),
    tilt: lerp(h.tilt, s.tilt, eT),
    beta: lerp(h.beta, s.beta, eB),
    rho: s.rho,
    phis: lerp(h.phis, s.phis, eArc),
    r: lerp(h.r, s.r, eArc),
    kappa: lerp(h.kappa, s.kappa, eArc),
    delta: lerp(h.delta, s.delta, eArc),
    yaw: lerp(h.yaw, s.yaw, eArc),
    lift: lerp(h.lift, s.lift, eArc),
    lead: lerp(h.lead, s.lead, eArc),
  };
}

// Neigung des Telefon-Stapels in Grad: dieselbe Blickrichtung wie der Fächer.
export const STACK_YAW = -28;

/**
 * Der Stapel am Telefon: die Karten rechts der Front staffeln sich schräg nach hinten, die
 * unsichtbaren links liegen dicht hinter ihr und öffnen sich erst im Morph. Jede Karte ist
 * deutlich kleiner als die davor (HUM-413).
 * @param {number} d @param {number} Ws @param {number} H
 * @returns {Pose}
 */
export function stackAt(d, Ws, H) {
  const s = d >= 0 ? d : -0.6 * d;
  return {
    x: Math.sign(d) * 0.15 * Ws * s,
    y: -0.035 * H * s,
    z: -0.32 * Ws * s,
    rz: 0,
    ry: STACK_YAW,
    rx: 0,
  };
}

/** Rang im Telefon-Stapel: d = +1 liegt direkt hinter der Front, d = -1 dahinter. @param {number} d */
export const rankOf = (d) => (d >= 0 ? (d <= 1 ? d : 2 * d - 1) : -2 * d);

/**
 * Pose am Platz d im Morph; ρ ist je Fenster gestaffelt.
 * @param {Choreo} c @param {Params} G @param {number} d @param {number} mm
 * @returns {Pose}
 */
export function morphPose(c, G, d, mm) {
  const mi = clamp((mm - 0.04 * Math.abs(d)) / 0.88);
  const rho = lerp(c.hero.rho, c.stage.rho, smoothstep(mi));
  if (!c.stack) return slotPose(G, d, rho, c.Ws);
  // Telefon hoch: der Stapel fächert sich zum Bogen auf, Kanal für Kanal.
  const h = stackAt(d, c.Ws, c.H);
  const s = slotPose(c.stage, d, rho, c.Ws);
  const rank = Math.min(3, rankOf(d));
  const e = clamp((mm - 0.08 * rank) / (1 - 0.24));
  const ex = easeInOutCubic(map(e, 0, 0.75));
  const ezy = smoothstep(map(e, 0.35, 1));
  // Äußere Karten bleiben außerhalb ihres inneren Nachbarn, er überholt sie nie.
  const inner = Math.abs(d) > 1.5 ? morphPose(c, G, d - Math.sign(d), mm).x : h.x;
  const x0 = d > 0 ? Math.max(h.x, inner) : Math.min(h.x, inner);
  /** @type {Pose} */
  const p = {
    x: lerp(x0, s.x, ex),
    y: lerp(h.y, s.y, ezy),
    z: lerp(h.z, s.z, ezy),
    rz: s.rz * ex,
    ry: lerp(h.ry, s.ry, ex),
    rx: 0,
  };
  // Die hinteren Karten bleiben im Morph hinter der Front, bis sie seitlich an ihr vorbei
  // sind. Auf der Bühne gilt allein der Vortritt (geometry.mjs): früher sprang die Tiefe am
  // Ende jeder Drehung von hinter der Front auf den Vortritt.
  if (rank > 0.01 && d > 0 && Math.abs(p.x) < 0.55 * c.Ws)
    p.z = lerp(Math.min(p.z, -0.05 * c.Ws), p.z, smoothstep(map(mm, 0.75, 1)));
  return p;
}

/**
 * Deckkraft am Platz d im Morph. Beide Enden sind vor ±3,5 bei 0, damit der Sprung über das
 * Ringende unsichtbar bleibt.
 * @param {Choreo} c @param {number} d @param {number} mm
 */
export function opacityOf(c, d, mm) {
  const a = Math.abs(d);
  const oh = profile(d >= 0 ? c.heroProfile : c.heroNeg, a);
  const os = profile(d >= 0 ? c.stagePos : c.stageNeg, a);
  return lerp(oh, os, smootherstep(map(mm, 0.05, 0.9)));
}
