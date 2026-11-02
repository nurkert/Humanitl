// Klassen des Bildausschnitts und ihre Vorgaben für Fächer und Bühne (HUM-334, HUM-413). Rein
// und ohne DOM, damit die Tests die Grenzen und Vorgaben prüfen können.

import { HERO_FAN, STAGE_POS } from "./choreography.mjs";
import { clamp } from "./math.mjs";

/** @typedef {"A" | "B" | "C" | "D" | "E" | "F" | "G"} Cls */
/** @typedef {import("./geometry.mjs").Params} Params */
/** @typedef {import("./math.mjs").Profile} Profile */

/**
 * Klasse des Bildausschnitts aus Breite und Höhe.
 * @param {number} vw @param {number} vh
 * @returns {Cls}
 */
export function classify(vw, vh) {
  const a = vw / vh;
  if (vh < 500 && a > 1.6) return "G";
  if (vw < 700 && a < 0.75) return "F";
  if (a >= 2) return "A";
  if (a > 1.6) return "B";
  if (a >= 1.45) return "C";
  if (a >= 1.15) return "D";
  if (vw >= 700) return "E";
  return "F";
}

// Der Fächer: die Front tritt vor, jede Scheibe dahinter liegt deutlich tiefer, damit der
// Stapel als klare Staffel liest und nicht als gedrängter Schwanz (Inhaber, 2026-11-02).
/** @type {Partial<Params>} */
const FAN = {
  tilt: 4,
  phis: 24,
  r: 1.3,
  kappa: 0.25,
  delta: 0.42,
  yaw: -30,
  lift: 0.1,
};

// Die Bühne (HUM-389, HUM-413): jedes Fenster frontal und waagrecht, die Front tritt deutlich
// vor, links von ihr stehen gehende Fenster vor ihr (lead). Steigung und Abstand sind die
// bevorzugten Werte; layout-solve.mjs nimmt den Abstand zurück und macht den Bogen steiler,
// bevor die Front kleiner wird.
export const LIFT = 0.22;
/** @type {Partial<Params>} */
export const FLAT = { kappa: 0, yaw: 0, rho: 0, lift: LIFT, lead: 1 };

// Wo die Front auf der Bühne steht: linker Rand als Anteil der Breite, null heißt mittig.
/** @type {Record<Cls, number | null>} */
export const EDGE = {
  A: 0.045,
  B: 0.04,
  C: 0.035,
  D: 0.03,
  E: null,
  F: null,
  G: 0.03,
};

/**
 * @typedef {object} ClassSpec
 * @property {(vw: number, vh: number, aspect: number) => number} ws gewünschte Breite der Front
 * @property {Partial<Params>} stage bevorzugte Bühne
 * @property {number} phisMin kleinster Winkelabstand
 * @property {number} betaMax steilste Steigung
 * @property {Partial<Params>} hero Fächer
 * @property {Profile} heroProfile
 * @property {Profile} stagePos
 * @property {number} morph Länge des Morphs in Bildhöhen
 */

/** @type {Record<Cls, ClassSpec>} */
export const SPEC = {
  A: {
    ws: (vw, vh) => Math.min(vw * 0.5, vh * 1.25),
    stage: { tilt: 2, beta: -18, phis: 30, r: 1.15, delta: 0.06 },
    phisMin: 18,
    betaMax: -26,
    hero: { ...FAN, phis: 22 },
    heroProfile: [
      [1, 1],
      [2, 0.85],
      [3, 0.5],
      [3.4, 0],
    ],
    stagePos: STAGE_POS,
    morph: 1,
  },
  B: {
    ws: (vw, vh) => Math.min(vw * 0.56, vh * 1.15),
    stage: { tilt: 2, beta: -18, phis: 30, r: 1.12, delta: 0.06 },
    phisMin: 18,
    betaMax: -26,
    hero: FAN,
    heroProfile: HERO_FAN,
    stagePos: STAGE_POS,
    morph: 1,
  },
  C: {
    ws: (vw, vh) => Math.min(vw * 0.58, vh * 1.05),
    stage: { tilt: 2, beta: -18, phis: 30, r: 1.12, delta: 0.06 },
    phisMin: 18,
    betaMax: -26,
    hero: FAN,
    heroProfile: HERO_FAN,
    stagePos: STAGE_POS,
    morph: 1,
  },
  D: {
    ws: (vw) => vw * 0.66,
    stage: { tilt: 3, beta: -16, phis: 30, r: 1.08, delta: 0.06 },
    phisMin: 18,
    betaMax: -24,
    hero: { ...FAN, phis: 18 },
    heroProfile: HERO_FAN,
    stagePos: STAGE_POS,
    morph: 1,
  },
  E: {
    ws: (vw) => vw * 0.84,
    stage: { tilt: 3, beta: -14, phis: 30, r: 1.15, delta: 0.06 },
    phisMin: 20,
    betaMax: -20,
    hero: { ...FAN, phis: 14, delta: 0.34 },
    heroProfile: HERO_FAN,
    stagePos: STAGE_POS,
    morph: 1,
  },
  F: {
    // Telefon hoch: die Front so breit, wie der Bildschirm erlaubt (layout-solve hält 10 px).
    ws: (vw) => vw * 0.92,
    stage: { tilt: 3, beta: -6, phis: 17, r: 2.4, delta: 0.05, lift: 0.08 },
    phisMin: 13,
    betaMax: -9,
    hero: { tilt: 4 },
    heroProfile: [
      [2, 1],
      [2.6, 0],
    ],
    stagePos: STAGE_POS,
    morph: 0.8,
  },
  G: {
    ws: (vw, vh, aspect) => Math.min(vw * 0.54, vh * 0.8 * aspect),
    stage: { tilt: 2, beta: -10, phis: 28, r: 1.2, delta: 0.05, lift: 0.16 },
    phisMin: 16,
    betaMax: -16,
    hero: { ...FAN, phis: 18, delta: 0.36 },
    // Telefon quer: rechts steht die Beschriftung, die Nachbarn rechts treten zurück.
    heroProfile: HERO_FAN,
    stagePos: [
      [0, 1],
      [0.5, 1],
      [0.9, 0],
    ],
    morph: 0.65,
  },
};

/** Text links, Stapel rechts (Breitbild, Telefon quer). @type {Set<Cls>} */
export const SIDE = new Set(["A", "B", "C", "G"]);
/** Beschriftung unter der Bühne (Hochformat). @type {Set<Cls>} */
export const BELOW = new Set(["E", "F"]);
/** Steuerung mittig unter der Front. @type {Set<Cls>} */
export const CTL_UNDER = new Set(["A", "B", "C", "D"]);

/** Abstand der Steuerung unter der Front. @param {number} vh */
export const ctlGapOf = (vh) => clamp(vh * 0.026, 16, 32);
