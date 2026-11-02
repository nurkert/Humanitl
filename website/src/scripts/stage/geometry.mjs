// Geometrie der Bühne: Parametersatz, Plätze auf dem Bogen, Projektion und Kästen. Rein und
// ohne DOM; alle Maße kommen über den Blick (View) herein.
//
// Die Geometrie hängt an einem Parametersatz G (Größe, Lage, Kippung, Rolle des Bogens,
// Winkelabstand, Radius). Der Morph mischt Parameter, nicht Posen: so bleibt die Reihenfolge
// der Fenster in jedem Zwischenzustand erhalten.

import { RAD, clamp, rotX, rotY, rotZ, smoothstep } from "./math.mjs";

/**
 * Parametersatz der Bühne; Längen in px, Winkel in Grad, r in Fensterbreiten.
 * @typedef {object} Params
 * @property {number} S Maßstab
 * @property {number} cx
 * @property {number} cy
 * @property {number} tilt Kippung um X
 * @property {number} beta Steigung des Bogens (rotateZ des Rigs)
 * @property {number} rho Wie weit die Fenster die Steigung mitrollen, 0 bis 1
 * @property {number} phis Winkelabstand der Plätze
 * @property {number} r Radius in Fensterbreiten
 * @property {number} kappa Wie weit die Fenster dem Bogen nachdrehen
 * @property {number} delta Zusätzliche Tiefe je Platz in Fensterbreiten
 * @property {number} yaw Grunddrehung um Y
 * @property {number} lift Wie weit das Fenster im Fokus vortritt, in Fensterbreiten
 * @property {number} lead 0 bis 1: links vom Fokus stehen die Fenster vor der Front statt
 *   hinter ihr (Bühne), damit ein gehendes Fenster nie hinter seinen Nachfolger taucht
 */

/**
 * Pose eines Fensters in Rig-Koordinaten. rx kippt um die Unterkante (Einflug).
 * @typedef {{ x: number, y: number, z: number, rz: number, ry: number, rx: number }} Pose
 */

/**
 * Was die Projektion braucht: Bildausschnitt, Fensterbreite und -höhe, Perspektive.
 * @typedef {{ vw: number, vh: number, Ws: number, H: number, P: number }} View
 */

/** @typedef {{ l: number, t: number, r: number, b: number }} Box */
/** @typedef {Box & { q: [number, number][] }} QuadBox */

// Breite des Glasrahmens in Fensterbreiten, wie --pad in Home.astro.
export const PAD = 0.005;

/**
 * Höhe eines Fensters: die Aufnahme mit ihrem Verhältnis plus der Rahmen oben und unten.
 * @param {number} Ws @param {number} aspect
 */
export const heightOf = (Ws, aspect) => (Ws * (1 - 2 * PAD)) / aspect + 2 * PAD * Ws;

/** Perspektive der Bühne; dieselbe Tiefe wirkt stärker, die Front löst sich vom Bogen. @param {number} Ws */
export const perspectiveOf = (Ws) => Math.max(1200, 2.4 * Ws);

/** Die Dicke muss im Fächer als Seitenwand zu lesen sein, auch verkleinert und geneigt. @param {number} Ws */
export const slabT = (Ws) => clamp(0.03 * Ws, 10, 30);

/**
 * Wie weit ein gehendes Fenster je Platz links vom Fokus nach vorn tritt, in Fensterbreiten.
 * Es bleibt so auf seinem ganzen Weg vor dem kommenden Fenster und blendet dabei aus; die
 * Ebenen tauschen nie die Reihenfolge (HUM-413).
 */
export const LEAD_FWD = 0.08;

/** @param {Partial<Params>} p @returns {Params} */
export function params(p) {
  return {
    S: 1,
    cx: 0,
    cy: 0,
    tilt: 0,
    beta: 0,
    rho: 1,
    phis: 20,
    r: 1.3,
    kappa: 1.15,
    delta: 0.08,
    yaw: 0,
    lift: 0,
    lead: 0,
    ...p,
  };
}

/**
 * Anteil des Vortritts am Platz d: am Fokus ganz, nach rechts bis d = 1 weich weg, nach links
 * bleibt er (das gehende Fenster tritt nicht zurück). Hängt nur an der Lage, nicht an der
 * Richtung einer Geste; so gibt es keinen Wechsel des Vortritts mitten in der Bewegung.
 * @param {number} d
 */
export const liftCurve = (d) => (d <= 0 ? 1 : 1 - smoothstep(clamp(d)));

/**
 * Platz auf dem Bogen in Rig-Koordinaten vor der Drehung des Fensters.
 * @param {Params} G @param {number} d @param {number} Ws
 * @returns {import("./math.mjs").Vec}
 */
export function slotVec(G, d, Ws) {
  const phi = d * G.phis * RAD;
  const R = G.r * Ws;
  const arcZ = -R * (1 - Math.cos(phi)) - G.delta * Math.abs(d) * Ws;
  const z = d < 0 ? arcZ + G.lead * (LEAD_FWD * -d * Ws - arcZ) : arcZ;
  return [R * Math.sin(phi), 0, z + G.lift * liftCurve(d) * Ws];
}

/**
 * @param {Params} G @param {number} d @param {number} rho @param {number} Ws
 * @returns {Pose}
 */
export function slotPose(G, d, rho, Ws) {
  const [x, y, z] = slotVec(G, d, Ws);
  return { x, y, z, rz: -(1 - rho) * G.beta, ry: -G.kappa * d * G.phis + G.yaw, rx: 0 };
}

/**
 * Rig-Transform auf einen lokalen Punkt: scale, rotateZ(β), rotateX(tilt), translate.
 * @param {Params} G @param {import("./math.mjs").Vec} p
 * @returns {import("./math.mjs").Vec}
 */
export function toWorld(G, p) {
  let v = /** @type {import("./math.mjs").Vec} */ ([p[0] * G.S, p[1] * G.S, p[2] * G.S]);
  v = rotZ(v, G.beta);
  v = rotX(v, G.tilt);
  return [v[0] + G.cx, v[1] + G.cy, v[2]];
}

/**
 * Perspektive der .world: Ursprung in der Mitte des Viewports.
 * @param {View} view @param {import("./math.mjs").Vec} v
 * @returns {[number, number]}
 */
export function project(view, v) {
  const k = view.P / Math.max(1, view.P - v[2]);
  return [view.vw / 2 + v[0] * k, view.vh / 2 + v[1] * k];
}

/**
 * Ein Punkt (u, v, w) eines Fensters in Rig-Koordinaten, wie der CSS-Transform ihn setzt.
 * @param {Pose} p @param {number} u @param {number} v @param {number} w @param {number} H
 * @returns {import("./math.mjs").Vec}
 */
export function winPoint(p, u, v, w, H) {
  /** @type {import("./math.mjs").Vec} */
  let q = [u, v, w];
  if (Math.abs(p.rx) > 0.01) {
    const hh = H / 2;
    q = rotX([q[0], q[1] - hh, q[2]], p.rx);
    q = [q[0], q[1] + hh, q[2]];
  }
  q = rotY(q, p.ry);
  q = rotZ(q, p.rz);
  return [q[0] + p.x, q[1] + p.y, q[2] + p.z];
}

/**
 * Projizierte Ecken eines Fensters mit Pose p unter G, im Umlaufsinn.
 * @param {View} view @param {Params} G @param {Pose} p
 * @returns {[number, number][]}
 */
export function quadWith(view, G, p) {
  return [
    [-1, -1],
    [1, -1],
    [1, 1],
    [-1, 1],
  ].map(([sx, sy]) =>
    project(view, toWorld(G, winPoint(p, (sx * view.Ws) / 2, (sy * view.H) / 2, 0, view.H))),
  );
}

/** @param {[number, number][]} q @returns {QuadBox} */
export function boxOf(q) {
  const xs = q.map((v) => v[0]);
  const ys = q.map((v) => v[1]);
  return { l: Math.min(...xs), t: Math.min(...ys), r: Math.max(...xs), b: Math.max(...ys), q };
}

/** @param {View} view @param {Params} G @param {Pose} p */
export const rectWith = (view, G, p) => boxOf(quadWith(view, G, p));

/**
 * Kasten des Fensters am Platz d auf einer flachen Bühne (ohne Morph).
 * @param {View} view @param {Params} G @param {number} d
 */
export const stageRect = (view, G, d) => rectWith(view, G, slotPose(G, d, G.rho, view.Ws));

/**
 * Trennachsentest: schneidet der Kasten c (mit Rand e) das konvexe Viereck q?
 * @param {Box} c @param {[number, number][]} q @param {number} [e]
 */
export function hitsQuad(c, q, e = 16) {
  /** @type {[number, number][]} */
  const rect = [
    [c.l - e, c.t - e],
    [c.r + e, c.t - e],
    [c.r + e, c.b + e],
    [c.l - e, c.b + e],
  ];
  /** @type {[number, number][]} */
  const axes = [
    [1, 0],
    [0, 1],
  ];
  for (let i = 0; i < 4; i++) {
    const [x0, y0] = q[i];
    const [x1, y1] = q[(i + 1) % 4];
    axes.push([y0 - y1, x1 - x0]);
  }
  for (const [ax, ay] of axes) {
    const pa = rect.map(([x, y]) => x * ax + y * ay);
    const pb = q.map(([x, y]) => x * ax + y * ay);
    if (Math.max(...pa) < Math.min(...pb) || Math.max(...pb) < Math.min(...pa)) return false;
  }
  return true;
}

/**
 * Abstand zweier Kästen in px, negativ, wenn sie sich überdecken (dann die kleinere
 * Eindringtiefe).
 * @param {Box} a @param {Box} b
 */
export function boxGap(a, b) {
  return Math.max(b.l - a.r, a.l - b.r, b.t - a.b, a.t - b.b);
}
