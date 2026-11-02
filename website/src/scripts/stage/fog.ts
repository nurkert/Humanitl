// Nebel und Lichtstrahlen hinter dem Stapel (HUM-341): Parallaxe im Morph und zum Zeiger,
// Einblenden je dekodiertem Bild, Drift im Compositor. Danach liegt der Nebel nicht mehr im
// Layout und belegt keine Ebenen.

import type { Ctx } from "./context";
import type { Params } from "./geometry.mjs";
import { clamp, map, smoothstep } from "./math.mjs";

// Bis hierher reicht der Nebel im Morph.
export const FOG_END = 0.3;
// Tiefe je Ebene für die Parallaxe, vorn bewegt sich mehr.
const DEPTH: Record<string, number> = { far: 0.25, rays: 0.6, near: 0.9 };

export function createFog(ctx: Ctx) {
  const { refs, s, put } = ctx;
  const { fog } = refs;
  const layers = [...fog.querySelectorAll<HTMLElement>("[data-fog-layer]")];
  let gone = false;

  function setGone(off: boolean) {
    if (off === gone) return;
    gone = off;
    fog.hidden = off;
  }

  /** Ein Bild: Deckkraft aus Einflug und Morph, Lage aus der Fahrt der Bühne. */
  function frame(G: Params, m: number, tIntro: number) {
    const off = m >= FOG_END;
    setGone(off);
    if (off) return;
    const fogIn = s.reduced ? 1 : s.introT0 < 0 ? 0 : clamp(tIntro / 1800);
    put(fog, "opacity", (fogIn * (1 - smoothstep(map(m, 0, FOG_END)))).toFixed(3));
    const hero = s.lay.hero;
    for (const layer of layers) {
      const depth = DEPTH[layer.dataset.fogLayer!];
      // Die Strahlen hängen wie der Lichtkegel an der Bühne, der Nebel nur an ihrer Fahrt.
      const x = depth === DEPTH.rays ? G.cx : G.cx - hero.cx;
      put(
        layer,
        "transform",
        `translate3d(${(depth * x).toFixed(1)}px, ${(depth * (G.cy - hero.cy)).toFixed(1)}px, 0)`,
      );
    }
  }

  function start() {
    if (!s.reduced) fog.dataset.drift = "";
    // Jede Ebene kommt, sobald ihr Bild dekodiert ist; der Einflug wartet nicht darauf. Far und
    // near haben eine Fassung je Thema: die andere lädt erst mit einem Themenwechsel (HUM-338).
    for (const tone of fog.querySelectorAll<HTMLElement>("[data-fog-tone]")) {
      const img = tone.querySelector("img")!;
      const show = () => {
        img
          .decode()
          .catch(() => undefined)
          .then(() => {
            tone.dataset.ready = "";
          });
      };
      if (img.complete && img.naturalWidth > 0) show();
      else img.addEventListener("load", show, { once: true });
    }
    // Ausgelaufen: Pose festschreiben, Animation und Ebenen-Hinweis entfernen.
    fog.addEventListener("animationend", (event) => {
      const img = event.target as HTMLElement;
      for (const animation of img.getAnimations()) {
        animation.commitStyles();
        animation.cancel();
      }
      img.closest<HTMLElement>("[data-fog-tone]")!.dataset.rest = "";
    });
    const pause = () => fog.toggleAttribute("data-paused", document.hidden);
    document.addEventListener("visibilitychange", pause);
    pause();

    // Der Zeiger schiebt die Ebenen um wenige Pixel; den weichen Lauf macht eine Transition auf
    // translate. Ganze Pixel und höchstens ein Schreiben je Bild halten das billig.
    if (s.reduced || !window.matchMedia("(hover: hover) and (pointer: fine)").matches) return;
    let pending = 0;
    let fx = 0;
    let fy = 0;
    window.addEventListener(
      "pointermove",
      (event) => {
        fx = event.clientX / s.vw - 0.5;
        fy = event.clientY / s.vh - 0.5;
        if (pending || s.lite || gone) return;
        pending = requestAnimationFrame(() => {
          pending = 0;
          for (const layer of layers) {
            const depth = DEPTH[layer.dataset.fogLayer!];
            put(
              layer,
              "translate",
              `${Math.round(-28 * depth * fx)}px ${Math.round(-16 * depth * fy)}px`,
            );
          }
        });
      },
      { passive: true },
    );
  }

  return {
    frame,
    start,
    /** Ein Sprung am Morph vorbei: ohne Bild käme der Nebel sonst nie aus dem Layout. */
    hide: () => setGone(true),
    get gone() {
      return gone;
    },
  };
}
