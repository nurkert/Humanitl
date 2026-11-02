// Kleine Mathematik der Bühne: Kurven, Federn, Drehungen. Rein und ohne DOM, damit die Tests
// sie ohne Übersetzer laden (wie follow.mjs).

/** @typedef {[number, number, number]} Vec */
/** @typedef {[number, number][]} Profile */

export const RAD = Math.PI / 180;

/** @param {number} v @param {number} [a] @param {number} [b] */
export const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
/** @param {number} a @param {number} b @param {number} t */
export const lerp = (a, b, t) => a + (b - a) * t;
/** Lage von v zwischen a und b, auf 0 bis 1 begrenzt. @param {number} v @param {number} a @param {number} b */
export const map = (v, a, b) => clamp((v - a) / (b - a));
/** @param {number} t */
export const smoothstep = (t) => t * t * (3 - 2 * t);
/** @param {number} t */
export const smootherstep = (t) => t * t * t * (t * (t * 6 - 15) + 10);
/** @param {number} t */
export const easeInOutCubic = (t) =>
  t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
/** @param {number} t */
export const easeInOutSine = (t) => -(Math.cos(Math.PI * t) - 1) / 2;
/** @param {number} t */
export const easeOutCubic = (t) => 1 - Math.pow(1 - t, 3);
/** @param {number} t */
export const easeOutQuad = (t) => 1 - (1 - t) * (1 - t);
/** @param {number} t */
export const easeInCubic = (t) => t * t * t;

/**
 * Stückweise lineare Kurve über |d|; nach dem letzten Punkt gilt dessen Wert.
 * @param {Profile} points
 * @param {number} x
 */
export function profile(points, x) {
  if (x <= points[0][0]) return points[0][1];
  for (let i = 1; i < points.length; i++) {
    const [x1, y1] = points[i];
    if (x <= x1) {
      const [x0, y0] = points[i - 1];
      return lerp(y0, y1, (x - x0) / (x1 - x0));
    }
  }
  return points[points.length - 1][1];
}

/**
 * Kubische Bézierkurve wie in CSS, gelöst mit wenigen Newton-Schritten.
 * @param {number} x1 @param {number} y1 @param {number} x2 @param {number} y2
 * @returns {(v: number) => number}
 */
export function bezier(x1, y1, x2, y2) {
  const cx = 3 * x1;
  const bx = 3 * (x2 - x1) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * y1;
  const by = 3 * (y2 - y1) - cy;
  const ay = 1 - cy - by;
  /** @param {number} t */
  const x = (t) => ((ax * t + bx) * t + cx) * t;
  /** @param {number} t */
  const dx = (t) => (3 * ax * t + 2 * bx) * t + cx;
  return (v) => {
    if (v <= 0) return 0;
    if (v >= 1) return 1;
    let t = v;
    for (let i = 0; i < 6; i++) {
      const d = dx(t);
      if (Math.abs(d) < 1e-6) break;
      t = clamp(t - (x(t) - v) / d);
    }
    return ((ay * t + by) * t + cy) * t;
  };
}

/**
 * Gedämpfte Feder in geschlossener Form, 0 bis 1; deterministisch über die Zeit in s.
 * @param {number} t @param {number} w @param {number} z
 */
export function springU(t, w, z) {
  if (t <= 0) return 0;
  if (z >= 1) return 1 - Math.exp(-w * t) * (1 + w * t);
  const wd = w * Math.sqrt(1 - z * z);
  return (
    1 -
    Math.exp(-z * w * t) * (Math.cos(wd * t) + ((z * w) / wd) * Math.sin(wd * t))
  );
}

// Drehungen wie die CSS-Funktionen rotateX/Y/Z (y zeigt nach unten, z zum Betrachter).
/** @param {Vec} v @param {number} a @returns {Vec} */
export function rotX([x, y, z], a) {
  const c = Math.cos(a * RAD);
  const s = Math.sin(a * RAD);
  return [x, y * c - z * s, y * s + z * c];
}
/** @param {Vec} v @param {number} a @returns {Vec} */
export function rotY([x, y, z], a) {
  const c = Math.cos(a * RAD);
  const s = Math.sin(a * RAD);
  return [x * c + z * s, y, -x * s + z * c];
}
/** @param {Vec} v @param {number} a @returns {Vec} */
export function rotZ([x, y, z], a) {
  const c = Math.cos(a * RAD);
  const s = Math.sin(a * RAD);
  return [x * c - y * s, x * s + y * c, z];
}
/** @param {Vec} v @returns {Vec} */
export const norm = ([x, y, z]) => {
  const l = Math.hypot(x, y, z) || 1;
  return [x / l, y / l, z / l];
};
/** @param {Vec} a @param {Vec} b */
export const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
