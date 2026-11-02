// Eingaben des Karussells: Pfeile, Tasten, Ziehen mit Maus und Finger, waagrechtes Scrollen,
// Zeiger und Tastaturfokus für den Selbstlauf (HUM-334, HUM-412).
//
// Senkrecht bleibt immer Sache der Seite: Ein Finger, der eher senkrecht läuft, scrollt
// (touch-action: pan-y), und eine Rad-Geste entscheidet ihre Achse einmal an ihren ersten
// Pixeln (stage/gesture.mjs). Wischen ist auf dem Telefon die erste Eingabe; der Ring folgt
// dem Finger 1:1 entlang der Bogentangente und rastet mit dem Schwung auf ein Fenster ein.

import type { Carousel } from "./carousel";
import type { Ctx } from "./context";
import { createWheelAxis, WHEEL_IDLE_MS, wheelPixels } from "./gesture.mjs";
import { RAD } from "./math.mjs";

export interface InputHooks {
  /** Steht der Punkt (Viewport) über einem sichtbaren Fenster? */
  overWindow(x: number, y: number): boolean;
  /** Steigung des Bogens im letzten Bild, in Grad. */
  beta(): number;
}

export function bindInput(ctx: Ctx, ring: Carousel, hooks: InputHooks) {
  const { s, refs } = ctx;
  const { scene, hit, deck, sticky } = refs;

  scene.querySelector("[data-prev]")!.addEventListener("click", () => ring.turn(-1));
  scene.querySelector("[data-next]")!.addEventListener("click", () => ring.turn(1));

  // Zeiger über einem Fenster hält den Selbstlauf; nur die Maus, ein Finger schwebt nicht.
  hit.addEventListener("pointermove", (event) => {
    if (event.pointerType === "mouse") ring.setHover(hooks.overWindow(event.clientX, event.clientY));
  });
  hit.addEventListener("pointerleave", () => ring.setHover(false));
  // Tastaturfokus in der Steuerung hält ihn auch; ein Mausklick auf einen Pfeil nicht.
  deck.addEventListener("focusin", (event) => {
    let visible = false;
    try {
      visible = (event.target as HTMLElement).matches(":focus-visible");
    } catch {
      /* ältere Browser kennen :focus-visible nicht */
    }
    ring.setKeyFocus(visible);
  });
  deck.addEventListener("focusout", (event) => {
    if (deck.contains(event.relatedTarget as Node | null)) return;
    ring.setKeyFocus(false);
  });
  // Pfeiltasten nur, wenn die Bühne mindestens die Hälfte des Viewports deckt.
  document.addEventListener("keydown", (event) => {
    if (!s.inView || !s.live || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey)
      return;
    const el = event.target as HTMLElement | null;
    if (el?.closest("input, textarea, select, [contenteditable]")) return;
    if (event.key === "ArrowLeft") ring.turn(-1);
    else if (event.key === "ArrowRight") ring.turn(1);
    else return;
    event.preventDefault();
  });

  // ---- Ziehen ---------------------------------------------------------------------
  let dragId = -1;
  let dragMoved = false;
  let ax = 0;
  let ay = 0;
  let pos0 = 0;
  let lastX = 0;
  let lastY = 0;
  let swallowClick = false;
  let samples: { t: number; p: number }[] = [];

  hit.addEventListener("pointerdown", (event) => {
    if (!s.live || s.dragging) return;
    if (event.pointerType === "mouse" && event.button !== 0) return;
    if ((event.target as Element).closest("button, a")) return;
    if (event.pointerType === "mouse") {
      // Sonst startet der Browser einen nativen Bild- oder Text-Drag und bricht ab.
      event.preventDefault();
      hit.setPointerCapture(event.pointerId);
    }
    s.dragging = true;
    dragMoved = false;
    dragId = event.pointerId;
    ax = event.clientX;
    ay = event.clientY;
  });

  hit.addEventListener("pointermove", (event) => {
    if (!s.dragging || event.pointerId !== dragId) return;
    lastX = event.clientX;
    lastY = event.clientY;
    const dx = event.clientX - ax;
    const dy = event.clientY - ay;
    if (!dragMoved) {
      if (Math.hypot(dx, dy) < 6) return;
      // Ein Finger, der eher senkrecht läuft, scrollt die Seite; der Ring rührt sich nicht.
      if (event.pointerType !== "mouse" && Math.abs(dy) >= Math.abs(dx)) {
        s.dragging = false;
        return;
      }
      // Ab der Schwelle gilt der ganze Weg seit dem Aufsetzen, damit der Ring 1:1 folgt.
      dragMoved = true;
      ring.hold();
      pos0 = s.pos;
      samples = [];
      s.steering = true;
      scene.toggleAttribute("data-dragging", true);
      ring.syncAuto();
    }
    const b = hooks.beta() * RAD;
    s.pos = pos0 - (dx * Math.cos(b) + dy * Math.sin(b)) / s.pxPerStep;
    ring.moved();
    const t = performance.now();
    samples.push({ t, p: s.pos });
    while (samples.length > 2 && t - samples[0].t > 80) samples.shift();
    ctx.schedule();
  });

  function release(cancelled: boolean) {
    if (!s.dragging) return;
    s.dragging = false;
    scene.toggleAttribute("data-dragging", false);
    if (!dragMoved) {
      ring.syncAuto();
      return;
    }
    s.steering = false;
    swallowClick = true;
    window.setTimeout(() => (swallowClick = false), 0);
    let v = 0;
    if (!cancelled && samples.length >= 2) {
      const a = samples[0];
      const z = samples[samples.length - 1];
      if (performance.now() - z.t < 80 && z.t > a.t) v = ((z.p - a.p) / (z.t - a.t)) * 1000;
    }
    ring.snap(v);
  }
  hit.addEventListener("pointerup", (e) => e.pointerId === dragId && release(false));
  hit.addEventListener("pointercancel", (e) => e.pointerId === dragId && release(true));
  hit.addEventListener("lostpointercapture", (e) => e.pointerId === dragId && release(false));
  scene.addEventListener(
    "click",
    (event) => {
      if (!swallowClick) return;
      swallowClick = false;
      event.stopPropagation();
      event.preventDefault();
    },
    true,
  );
  hit.addEventListener("dragstart", (event) => event.preventDefault());

  // ---- Rad ------------------------------------------------------------------------
  // Waagrechtes Trackpad-Scrollen dreht live; die Achse gilt für die ganze Geste samt
  // Nachlauf, entschieden an ihren ersten Pixeln.
  const wheelAxis = createWheelAxis();
  let wheelTimer = 0;
  sticky.addEventListener(
    "wheel",
    (event) => {
      const [px, py] = wheelPixels(event, s.vw);
      const axis = wheelAxis(px, py, performance.now());
      if (axis === "y" || !s.live) return;
      if (axis === null) {
        // Noch offen: nicht drehen, aber eine waagrechte Geste nicht als Blättern im Verlauf
        // an den Browser geben.
        if (Math.abs(px) > Math.abs(py)) event.preventDefault();
        return;
      }
      event.preventDefault();
      ring.hold();
      s.steering = true;
      s.pos += px / s.pxPerStep;
      ring.moved();
      ring.syncAuto();
      clearTimeout(wheelTimer);
      wheelTimer = window.setTimeout(() => {
        s.steering = false;
        ring.snap(0);
      }, WHEEL_IDLE_MS);
      ctx.schedule();
    },
    { passive: false },
  );

  return {
    /** Größenänderung mitten im Ziehen: von hier aus mit dem neuen Maßstab, ohne Sprung. */
    rebase() {
      if (dragMoved && s.dragging) {
        pos0 = s.pos;
        ax = lastX;
        ay = lastY;
      }
    },
  };
}
