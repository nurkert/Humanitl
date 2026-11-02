// Posen und Deckkraft aller Fenster für einen Stand des Morphs (HUM-413). Rein und ohne DOM:
// frame.ts ruft es je Bild auf und schreibt nur noch, die Tests rechnen dieselben Stände nach.
//
// Die sieben Module stehen nach der Lage des Rings (ringAt), dazu kommt als Eintrag n das
// Fenster mit dem Ablauf: Es steht oben auf Platz 0, rückt mit dem Ring einen Platz nach links
// und geht dabei ganz aus dem Bild, ohne auszublenden. Auf der Bühne gibt es es nicht.

import {
  REWIND,
  SHIFT,
  exitAmount,
  exitShift,
  moduleShown,
  morphPose,
  opacityOf,
  ringAt,
  wrapRing,
} from "./choreography.mjs";
import { project, toWorld, winPoint } from "./geometry.mjs";
import { clamp, lerp, map, smoothstep } from "./math.mjs";

/** @typedef {import("./geometry.mjs").Pose} Pose */
/** @typedef {import("./geometry.mjs").Params} Params */
/** @typedef {import("./geometry.mjs").View} View */

/**
 * @typedef {object} RingState
 * @property {number} n Zahl der Module
 * @property {number} pos gleitende Lage des Rings
 * @property {number} target ganzzahliges Ziel des Rings
 * @property {number} steps Schritte beim Zurückscrollen, mit Vorzeichen (rewindSteps)
 * @property {number} m Morph
 * @property {boolean} lead gibt es das Fenster mit dem Ablauf?
 * @property {boolean} reduced reduzierte Bewegung
 */

/**
 * Plätze, Posen und Deckkraft aller Fenster; das Fenster mit dem Ablauf ist, wenn vorhanden,
 * der letzte Eintrag.
 * @param {import("./choreography.mjs").Choreo} c @param {Params} G @param {View} view
 * @param {RingState} r
 * @returns {{ ds: number[], poses: Pose[], ops: number[], exit: number, exiting: boolean }}
 */
export function assemble(c, G, view, r) {
  const { n, m } = r;
  const at = ringAt(r.pos, r.target, r.steps, m);
  const ds = Array.from({ length: n }, (_, i) => wrapRing(i - at, n));
  const exit = r.reduced ? (m >= 0.5 ? 1 : 0) : exitAmount(m);
  const exiting = r.lead && m < REWIND[0];
  if (r.lead) ds.push(-smoothstep(map(m, SHIFT[0], SHIFT[1])));
  /** @type {Pose[]} */
  const poses = [];
  /** @type {number[]} */
  const ops = [];
  ds.forEach((d, i) => {
    let p = morphPose(c, G, d, m);
    let o;
    if (i === n) {
      if (exit > 0) p = { ...p, x: p.x - exit * exitShift(view, G, p) };
      o = exiting && exit < 1 ? 1 : 0;
    } else o = moduleShown(d, m) ? opacityOf(c, d, m) : 0;
    poses.push(p);
    ops.push(o);
  });
  return { ds, poses, ops, exit, exiting };
}

/**
 * Fenster jenseits von d = +1 blenden aus, sobald sie in das Band der Navigation steigen: dort
 * schimmerte der Bildschirm durch die halbtransparente Leiste. Ändert ops an Ort und Stelle.
 * @param {number[]} ds @param {Pose[]} poses @param {number[]} ops @param {Params} G
 * @param {View} view @param {number} navBottom
 */
export function fadeUnderNav(ds, poses, ops, G, view, navBottom) {
  const band = 0.05 * view.vh + 24;
  ds.forEach((d, i) => {
    const wgt = smoothstep(map(Math.abs(d), 1, 2));
    if (wgt <= 0 || ops[i] <= 0) return;
    const p = poses[i];
    const top = Math.min(
      ...[-1, 1].map(
        (sx) =>
          project(view, toWorld(G, winPoint(p, (sx * view.Ws) / 2, -view.H / 2, 0, view.H)))[1],
      ),
    );
    ops[i] *= lerp(1, clamp((top - (navBottom + 24 - band)) / band), wgt);
  });
}

/**
 * Lage der Fenstermitten auf dem Schirm und Rang von links nach rechts unter den sichtbaren
 * Fenstern, gezählt ab der Front (für das Begleitlicht).
 * @param {number[]} ds @param {Pose[]} poses @param {number[]} ops @param {Params} G
 * @param {View} view
 * @returns {{ screenX: number[], rank: number[] }}
 */
export function ranks(ds, poses, ops, G, view) {
  const screenX = poses.map((p) => project(view, toWorld(G, [p.x, p.y, p.z]))[0]);
  const byX = ds.map((_, i) => i).sort((a, b) => screenX[a] - screenX[b]);
  const order = new Array(ds.length).fill(0);
  let k = 0;
  for (const i of byX) if (ops[i] > 0.01) order[i] = k++;
  const front = order[ds.findIndex((d, i) => Math.abs(d) < 0.5 && ops[i] > 0.01)] ?? 0;
  return { screenX, rank: order.map((v) => Math.abs(v - front)) };
}
