// Licht auf den Fenstern (HUM-348): Schatten, Kantenlicht, Glanzpunkt und Begleitlicht im
// Morph. Pro Bild ändern sich nur Custom Properties, die Deckkraft und Verschiebung fertiger
// Lichtebenen steuern.

import type { Ctx } from "./context";
import type { Params, Pose } from "./geometry.mjs";
import {
  RAD,
  clamp,
  dot,
  lerp,
  map,
  norm,
  rotX,
  rotY,
  rotZ,
  smootherstep,
  smoothstep,
  type Vec,
} from "./math.mjs";

// Weltlicht: oben vorn leicht links, fest im Raum.
const LIGHT = norm([-0.3, -0.85, 0.45]);
const LIGHT_FLAT = norm([LIGHT[0], 0, LIGHT[2]]);
// Glanzstellung für den Rim und den Glint: knapp links oberhalb der Blickachse, damit die
// Front im Ruhezustand glänzt und jedes Fenster beim Drehen durch die Glanzstellung läuft.
const SPEC_H = norm([-0.1, -0.3, 1]);

export interface LightInput {
  G: Params;
  p: Pose;
  d: number;
  /** Lage der Fenstermitte auf dem Schirm in px. */
  sx: number;
  m: number;
  /** Rang von links nach rechts unter den sichtbaren Fenstern, gezählt ab der Front. */
  rank: number;
  /** Deckkraft der Blätter und Helligkeit. */
  a0: number;
  b0: number;
  /** Anteil der Unschärfe. */
  soft: number;
}

/**
 * Gedimmte Fenster werden dunkel, nicht durchsichtig: sonst schimmern die hinteren Konturen
 * durch den Bildschirm.
 */
export function paintLight(ctx: Ctx, win: HTMLElement, i: LightInput) {
  const { s, put } = ctx;
  const { lay } = s;
  const { p, G, d, m } = i;
  let nv: Vec = [0, 0, 1];
  nv = rotX(nv, p.rx);
  nv = rotY(nv, p.ry);
  nv = rotZ(nv, p.rz);
  nv = rotZ(nv, G.beta);
  nv = rotX(nv, G.tilt);
  const diffuse = clamp(0.35 + 0.65 * dot(nv, LIGHT_FLAT));
  const spec = s.reduced ? 0.4 : Math.pow(Math.max(0, dot(nv, SPEC_H)), 14);
  const a = Math.abs(d);
  let shade = Math.min(0.72, 0.55 * (1 - diffuse) + 0.15 * Math.max(0, a - 0.5));
  if (a < 0.5) shade = Math.min(shade, 0.05);
  if (lay.stack) {
    // Im Stapel liegen die hinteren Karten im Schatten der Front.
    const stackShade = Math.min(0.72, 0.26 * a);
    shade = lerp(stackShade, shade, smoothstep(map(m, 0.2, 0.9)));
  }
  const yaw = p.ry;
  // Die Kante leuchtet, sobald das Fenster aus der Blickachse dreht; frontal bleibt sie ruhig.
  const turned = Math.hypot(Math.sin(yaw * RAD), Math.sin(p.rx * RAD) * Math.cos(yaw * RAD));
  const lit = clamp(turned * 1.4) * (0.5 + 0.5 * diffuse);
  const gx = (clamp(-yaw / 45, -1, 1) * 0.6 + (0.25 * (i.sx - s.vw / 2)) / s.vw) * lay.Ws;
  const glint = 0.18 + 0.77 * spec;
  // Begleitlicht im Morph: läuft von links nach rechts über die Fenster, die Front zuerst.
  let swx = 0;
  let swo = 0;
  if (!s.reduced) {
    const k = smoothstep(map(m, 0.45 + 0.05 * i.rank, 0.85 + 0.05 * i.rank));
    swx = (2 * k - 1) * lay.Ws;
    swo = 4 * k * (1 - k);
  }
  // Der Schein gehört zur Front. Ein gehendes Fenster nimmt ihn sofort mit weg: auf Hell stand
  // er sonst als blasse Platte links neben der Bühne.
  const halo =
    (1 - clamp(d < 0 ? a / 0.25 : a)) * (0.55 + 0.45 * smootherstep(map(m, 0.05, 0.9)));
  put(win, "--shade", (1 - (1 - shade) * i.b0).toFixed(3));
  // Unscharfe Fenster tragen keine harte Lichtkante.
  put(win, "--lit", (lit * i.a0 * i.a0 * i.b0 * (1 - 0.8 * i.soft)).toFixed(3));
  put(win, "--spec", glint.toFixed(3));
  put(win, "--gx", `${gx.toFixed(1)}px`);
  put(win, "--swx", `${swx.toFixed(1)}px`);
  put(win, "--swo", swo.toFixed(3));
  put(win, "--halo", halo.toFixed(3));
}
