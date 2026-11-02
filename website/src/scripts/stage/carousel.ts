// Der Ring des Karussells: Kurven je Wechsel, Feder nach Ziehen und Rad, und der Selbstlauf
// mit den Abschnitten der Steuerung (HUM-334, HUM-389, HUM-412).
//
// Ein Wechsel per Pfeil, Taste oder Selbstlauf ist eine einzige Kurve für alle Fenster. Ein
// weiterer Klick mitten in der Fahrt hängt eine zweite Kurve an, die sich zur ersten addiert:
// so ändert sich das Ziel ohne Sprung in Lage oder Richtung.

import type { Ctx } from "./context";
import { bezier, clamp } from "./math.mjs";

// Dauer und Kurve eines Wechsels: schnell los, lang und weich aus.
const TURN_MS = 720;
const easeTurn = bezier(0.22, 1, 0.36, 1);

// Der aktive Abschnitt füllt sich in AUTO_MS, dann dreht der Ring eins weiter.
const AUTO_MS = 5000;
// Ein neu gestarteter Abschnitt läuft erst in dieser Zeit leer, statt auf 0 zu springen.
const DRAIN_MS = 240;

export function createCarousel(ctx: Ctx, onSettled: (auto: boolean) => void) {
  const { s, refs } = ctx;
  const { segs } = refs;
  let vel = 0;
  let target = s.pos;
  let springOn = false;
  let tweens: { t0: number; delta: number }[] = [];
  // Seit dem ersten Klick liefen mehrere Kurven zugleich; erst wenn alle stehen, wechselt die
  // Beschriftung.
  let multiTurn = false;
  let tweenBase = s.pos;
  let lastTurnAuto = false;

  function settled() {
    onSettled(lastTurnAuto);
    syncAuto();
  }

  function springStep(h: number) {
    const w = 11;
    const z = 0.86;
    const a = -w * w * (s.pos - target) - 2 * z * w * vel;
    vel += a * h;
    s.pos += vel * h;
    if (Math.abs(s.pos - target) < 0.0005 && Math.abs(vel) < 0.002) {
      // Präzises Einrasten: am Ende steht pos exakt auf dem Ziel.
      s.pos = target;
      vel = 0;
      springOn = false;
      settled();
    }
  }

  function tweenStep(now: number) {
    tweens = tweens.filter((tw) => {
      if (now - tw.t0 < TURN_MS) return true;
      tweenBase += tw.delta;
      return false;
    });
    if (!tweens.length) {
      multiTurn = false;
      s.pos = target;
      tweenBase = target;
      settled();
      return;
    }
    let p = tweenBase;
    for (const tw of tweens) p += tw.delta * easeTurn((now - tw.t0) / TURN_MS);
    s.pos = p;
  }

  /** Ein Bild der Bewegung; liefert true, solange sich der Ring dreht. */
  function step(now: number, dt: number): boolean {
    let busy = false;
    if (springOn) {
      let left = dt;
      while (left > 1e-6 && springOn) {
        const h = Math.min(left, 1 / 30);
        springStep(h);
        left -= h;
      }
      busy ||= springOn;
    }
    if (tweens.length) {
      tweenStep(now);
      busy ||= tweens.length > 0;
    }
    return busy;
  }

  /** Bricht Kurven und Feder ab; der Ring bleibt, wo er gerade ist. */
  function hold() {
    tweens = [];
    multiTurn = false;
    tweenBase = s.pos;
    springOn = false;
    vel = 0;
  }

  function turn(by: number, auto = false) {
    if (!s.live) return;
    s.steering = false;
    lastTurnAuto = auto;
    // Nach Ziehen oder Wischen zählt die Position; während eine Fahrt läuft, zählt ein
    // weiterer Klick von ihrem Ziel aus weiter.
    if (!tweens.length) {
      hold();
      target = Math.round(s.pos);
    }
    const goal = clamp(target + by, Math.round(s.pos) - 3, Math.round(s.pos) + 3);
    const dest = tweens.reduce((sum, tw) => sum + tw.delta, tweenBase);
    target = goal;
    if (s.reduced) {
      hold();
      s.pos = target;
      tweenBase = target;
      settled();
    } else if (Math.abs(goal - dest) > 1e-6) {
      tweens.push({ t0: performance.now(), delta: goal - dest });
      if (tweens.length > 1) multiTurn = true;
    }
    // Ein Eingriff von Hand startet die Zeit neu, auch wenn er den Fokus am Ende nicht wechselt.
    if (!auto) restartAuto();
    ctx.schedule();
  }

  /** Nach Ziehen oder Rad: einrasten auf das nächste Fenster, mit dem Schwung v in Fenster/s. */
  function snap(v: number) {
    lastTurnAuto = false;
    // Der Ring steht, wo der Zeiger ihn hingezogen hat; begrenzt wird nur der Schwung danach,
    // höchstens drei Fenster weiter (HUM-334).
    target = Math.round(s.pos + clamp(v * 0.32, -3, 3));
    if (s.reduced) {
      s.pos = target;
      vel = 0;
      settled();
    } else {
      vel = v;
      springOn = true;
    }
    restartAuto();
    ctx.schedule();
  }

  /**
   * Die Bühne verlässt das Bild: Kurven, Feder, Zug und Rad halten an, wo der Ring gerade zu
   * sehen ist, und diese Lage wird das Ziel. So dreht der Ring beim Zurückscrollen von dem
   * Stand aus, den man sieht, und nicht von einem Ziel, das ein Wurf erst noch erreicht hätte.
   */
  function stop() {
    s.dragging = false;
    s.steering = false;
    refs.scene.toggleAttribute("data-dragging", false);
    hold();
    target = s.pos;
  }

  /** Setzt den Ring ohne Bewegung auf eine Lage (oben im Stapel, unsichtbar). */
  function jump(to: number) {
    hold();
    s.pos = to;
    target = to;
    tweenBase = to;
  }

  // ---- Selbstlauf -----------------------------------------------------------------
  // Er hält, solange der Zeiger über einem Fenster steht, die Tastatur in der Steuerung ist,
  // der Tab verborgen oder die Bühne aus dem Bild ist, und bei reduzierter Bewegung läuft er
  // nie. Die Füllung ist eine Web Animation: pausiert hält sie ihren Stand, und pro Bild rechnet
  // kein Skript.
  const autoOn =
    segs.length === s.n && typeof segs[0]?.animate === "function";
  let autoAnim: Animation | null = null;
  let hover = false;
  let keyFocus = false;

  const inMotion = () => tweens.length > 0 || springOn || s.dragging || s.steering;

  const autoAllowed = () =>
    s.live && s.inView && !document.hidden && !hover && !keyFocus && !inMotion();

  // Ohne den Cache von put(): commitStyles() schreibt den Stil an ihm vorbei. Die Werte gehen
  // über eine Transition im CSS, so springt beim Wechsel kein Abschnitt.
  function paintSegs() {
    segs.forEach((seg, i) => {
      seg.style.transform =
        i < s.active || ((!autoOn || s.reduced) && i === s.active) ? "scaleX(1)" : "scaleX(0)";
    });
  }

  /** Füllstand eines Abschnitts, wie er gerade gemalt ist. */
  function segScale(seg: HTMLElement): number {
    const t = getComputedStyle(seg).transform;
    return !t || t === "none" ? 0 : new DOMMatrixReadOnly(t).a;
  }

  /** Neuer Abschnitt oder Eingriff von Hand: die Füllung beginnt von vorn. */
  function restartAuto() {
    // Den gemalten Stand übernehmen, bevor die Animation geht; sonst fiele der alte Abschnitt
    // für ein Bild auf seinen Grundwert zurück und liefe dann erst zum neuen.
    const from = autoAnim ? segScale(segs[s.active]) : 0;
    if (autoAnim) {
      try {
        autoAnim.commitStyles();
      } catch {
        /* nicht gerenderte Elemente können nichts übernehmen */
      }
      autoAnim.cancel();
    }
    autoAnim = null;
    paintSegs();
    if (!autoOn || s.reduced || !s.live) return;
    const drain = from > 0.01 ? DRAIN_MS : 0;
    const total = AUTO_MS + drain;
    const anim = segs[s.active].animate(
      drain
        ? [
            { transform: `scaleX(${from.toFixed(3)})` },
            { transform: "scaleX(0)", offset: drain / total },
            { transform: "scaleX(1)" },
          ]
        : [{ transform: "scaleX(0)" }, { transform: "scaleX(1)" }],
      { duration: total, easing: "linear", fill: "forwards" },
    );
    autoAnim = anim;
    anim.pause();
    anim.onfinish = () => {
      if (autoAnim === anim && autoAllowed()) turn(1, true);
    };
    syncAuto();
  }

  function syncAuto() {
    const anim = autoAnim;
    if (!anim) return;
    const run = autoAllowed();
    if (run && anim.playState === "finished") turn(1, true);
    else if (run && anim.playState === "paused") anim.play();
    else if (!run && anim.playState === "running") anim.pause();
  }

  return {
    step,
    hold,
    turn,
    snap,
    stop,
    jump,
    inMotion,
    restartAuto,
    syncAuto,
    paintSegs,
    setHover(on: boolean) {
      if (on === hover) return;
      hover = on;
      syncAuto();
    },
    setKeyFocus(on: boolean) {
      keyFocus = on;
      syncAuto();
    },
    /** Ziehen oder Rad schieben den Ring; Kurven gehen in die neue Lage über. */
    moved() {
      tweenBase = s.pos;
    },
    get target() {
      return target;
    },
    get vel() {
      return vel;
    },
    get springOn() {
      return springOn;
    },
    get tweening() {
      return tweens.length > 0;
    },
    /** Mehrere Schritte laufen oder liefen in dieser Fahrt: die Beschriftung wartet. */
    get multiTurn() {
      return multiTurn;
    },
    get auto() {
      return autoAnim
        ? { state: autoAnim.playState, t: autoAnim.currentTime, hover, keyFocus }
        : null;
    },
  };
}

export type Carousel = ReturnType<typeof createCarousel>;
