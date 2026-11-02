// Der Einflug der Fenster beim Laden (HUM-334, HUM-348): jedes Fenster startet aus einer
// eigenen Pose und landet über Federn je Kanal; was im Stapel unsichtbar steht, blendet nur auf.

import { morphPose, wrapRing } from "./choreography.mjs";
import type { Ctx } from "./context";
import type { Pose } from "./geometry.mjs";
import { clamp, lerp, springU } from "./math.mjs";

// Startzeiten je Abstand d im Stapel, von vorn nach hinten; jedes spätere Fenster kommt hinter
// schon gelandeten an. Was im Fächer unsichtbar steht, fliegt nie.
const FAN_DELAY: Record<string, number> = { "0": 0, "1": 300, "2": 540, "3": 740 };
const STACK_DELAY: Record<string, number> = { "2": 0, "1": 160, "0": 320 };

export function createIntro(ctx: Ctx) {
  const { s } = ctx;
  let starts: Pose[] = [];
  let delays: number[] = [];

  /**
   * Startposen aus der Lage des Rings oben im Stapel (ring: Index auf Platz 0). Platz 0 gehört
   * dem Fenster mit dem Ablauf (Eintrag n); ein Modul dort und links davon fliegt nicht.
   */
  function prepare(ring: number) {
    const { lay } = s;
    const G = lay.hero;
    starts = [];
    delays = [];
    const count = s.n + (ctx.refs.lead ? 1 : 0);
    for (let i = 0; i < count; i++) {
      const d = i === s.n ? 0 : Math.round(wrapRing(i - ring, s.n));
      const P = morphPose(lay, G, d, 0);
      const key = i < s.n && d <= 0 ? "hidden" : String(d);
      let start: Pose;
      let delay: number;
      if (lay.stack) {
        delay = STACK_DELAY[key] ?? -1;
        start = { ...P, y: P.y + (s.vh * 0.1) / G.S, rx: 28 };
      } else if (key === "0") {
        delay = 0;
        start =
          lay.cls === "G"
            ? { ...P, x: P.x + (s.vw * 0.06) / G.S, ry: P.ry - 14 }
            : {
                ...P,
                y: P.y + (s.vh * 0.3) / G.S,
                z: P.z - 0.5 * lay.Ws,
                rx: 44,
                ry: P.ry - 10,
              };
      } else {
        delay = FAN_DELAY[key] ?? -1;
        start = {
          ...P,
          x: 0.15 * P.x,
          y: P.y + (s.vh * 0.08) / G.S,
          z: P.z - 0.35 * lay.Ws,
          rx: 14,
          ry: 0.2 * P.ry,
        };
      }
      starts.push(start);
      delays.push(delay);
    }
  }

  /** Mischt Start und Ziel je Kanal mit seiner Feder; liefert Pose und Deckkraft. */
  function blend(i: number, P: Pose, now: number): [Pose, number] {
    const delay = delays[i] ?? -1;
    const t0 = s.introT0 + (delay < 0 ? 820 : delay);
    const t = (now - t0) / 1000;
    if (delay < 0 || !starts[i]) return [P, clamp(t / 0.42)];
    if (t <= 0) return [starts[i], 0];
    const uT = springU(t, 6.5, 1);
    const uR = springU(t, 5.2, 0.8);
    const st = starts[i];
    return [
      {
        x: lerp(st.x, P.x, uT),
        y: lerp(st.y, P.y, uT),
        z: lerp(st.z, P.z, uT),
        rz: P.rz,
        ry: lerp(st.ry, P.ry, uR),
        rx: lerp(st.rx, P.rx, uR),
      },
      clamp(t / 0.42),
    ];
  }

  /** Ist Fenster i schon gelandet (2 s nach seinem Start)? */
  const landed = (i: number, now: number) =>
    now - s.introT0 - Math.max(0, delays[i] ?? 0) >= 2000;

  return { prepare, blend, landed };
}

export type Intro = ReturnType<typeof createIntro>;
