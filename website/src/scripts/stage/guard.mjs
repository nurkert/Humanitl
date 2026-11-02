// Schutz gegen Durchdringung im Fächer (HUM-348). Rein und ohne DOM.
//
// Im Fächer sind die Fenster um Y gedreht; ihre inneren Kanten können in den Nachbarn ragen.
// Für jedes Nachbarpaar weicht das äußere Fenster nach hinten aus, bis seine Vorderseite hinter
// der Rückwand des inneren liegt. Auf der Bühne stehen alle Fenster parallel und ihre Tiefe
// ist so gewählt, dass nichts durchdringt; dort schöbe der Schutz ein gehendes Fenster hinter
// seinen Nachfolger und tauschte die Ebenen (HUM-413). Darum wirkt er nur mit dem Gewicht
// weight, das im Morph auf 0 fällt.

import { dot, map, smoothstep } from "./math.mjs";
import { winPoint } from "./geometry.mjs";

/** @typedef {import("./geometry.mjs").Pose} Pose */

/**
 * Wie weit die innere Kante von b (Ecken oben, Mitte, unten) vor der Ebene von a liegt,
 * gezählt nur dort, wo sie über a steht. Liefert die Tiefe und die z-Komponente der Normale
 * von a, um eine Tiefe in eine Verschiebung nach hinten umzurechnen.
 * @param {Pose} b @param {number} u @param {Pose} a @param {number} Ws @param {number} H
 * @returns {[number, number]}
 */
export function penetration(b, u, a, Ws, H) {
  const half = Ws / 2;
  const hh = H / 2;
  const c = winPoint(a, 0, 0, 0, H);
  const ex = winPoint(a, 1, 0, 0, H);
  const ez = winPoint(a, 0, 0, 1, H);
  /** @type {import("./math.mjs").Vec} */
  const tx = [ex[0] - c[0], ex[1] - c[1], ex[2] - c[2]];
  /** @type {import("./math.mjs").Vec} */
  const nz = [ez[0] - c[0], ez[1] - c[1], ez[2] - c[2]];
  let depth = -Infinity;
  for (const v of [-hh, 0, hh]) {
    const k = winPoint(b, u, v, 0, H);
    /** @type {import("./math.mjs").Vec} */
    const rel = [k[0] - c[0], k[1] - c[1], k[2] - c[2]];
    if (Math.abs(dot(rel, tx)) > half + 2) continue;
    depth = Math.max(depth, dot(rel, nz));
  }
  return [depth, Math.max(0.3, nz[2])];
}

/**
 * Schiebt in poses (in place) je Nachbarpaar das äußere Fenster nach hinten. Mitten in einer
 * Drehung wechselt, welches Fenster außen ist; dort geht die Verschiebung stetig von einem auf
 * das andere über. Liefert die größte Verschiebung.
 * @param {Pose[]} poses @param {number[]} ds
 * @param {{ Ws: number, H: number, T: number, weight: number }} o
 */
export function guard(poses, ds, o) {
  if (o.weight <= 0) return 0;
  const margin = o.T + 6;
  const half = o.Ws / 2;
  const idx = ds
    .map((_, i) => i)
    .filter((i) => Math.abs(ds[i]) <= 3.2)
    .sort((a, b) => ds[a] - ds[b]);
  /** @type {[number, number][]} */
  const pairs = [];
  for (let k = 0; k + 1 < idx.length; k++) {
    const a = idx[k];
    const b = idx[k + 1];
    if (Math.abs(ds[b] - ds[a] - 1) < 0.01) pairs.push([a, b]);
  }
  const inner = (/** @type {[number, number]} */ q) =>
    Math.min(Math.abs(ds[q[0]]), Math.abs(ds[q[1]]));
  pairs.sort((p, q) => inner(p) - inner(q));
  let most = 0;
  for (const [a, b] of pairs) {
    // a liegt links von b; w ist das Gewicht dafür, dass b das äußere Fenster ist.
    const w = smoothstep(map(Math.abs(ds[b]) - Math.abs(ds[a]), -0.3, 0.3));
    const [db, nb] = penetration(poses[b], -half, poses[a], o.Ws, o.H);
    const [da, na] = penetration(poses[a], half, poses[b], o.Ws, o.H);
    const pushB = o.weight * w * Math.max(0, db + margin);
    const pushA = o.weight * (1 - w) * Math.max(0, da + margin);
    poses[b].z -= pushB / nb;
    poses[a].z -= pushA / na;
    most = Math.max(most, pushA, pushB);
  }
  return most;
}
