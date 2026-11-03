// Bildausschnitt und Maße der Bühne (HUM-334, HUM-413): Klasse je Seitenverhältnis, Fächer der
// Startseite und Lage des Karussells. Liest das DOM nur hier, einmal je Größenänderung; die
// Rechnung selbst steht in layout-solve.mjs.

import { HERO_NEG, STAGE_NEG, stackAt } from "./choreography.mjs";
import {
  BELOW,
  CTL_UNDER,
  EDGE,
  FLAT,
  SIDE,
  SPEC,
  H_BOUNDS,
  ctlGapOf,
  heightMarks,
  nextClass,
} from "./classes.mjs";
import type { Cls, Ctx, Layout } from "./context";
import {
  type Box,
  type Params,
  heightOf,
  params,
  perspectiveOf,
  rectWith,
  slabT,
  stageRect,
} from "./geometry.mjs";
import { clamp, profile, type Profile } from "./math.mjs";
import { solveStage } from "./layout-solve.mjs";

/** Seitenverhältnis der Aufnahmen, aus dem importierten Bild (data-aspect an der Szene). */
export const aspectOf = (scene: HTMLElement) =>
  Number(scene.dataset.aspect) || 2560 / 1352;


/** Lage eines Elements im angehefteten Abschnitt, ohne Transform (offsetTop-Kette). */
function offsetIn(el: HTMLElement, sticky: HTMLElement): number {
  let y = 0;
  let node: HTMLElement | null = el;
  while (node && node !== sticky) {
    y += node.offsetTop;
    node = node.offsetParent as HTMLElement | null;
  }
  return y;
}

/**
 * Kasten eines Elements im angehefteten Abschnitt über die offset-Kette: ohne Transform (der
 * Kopf der Beschriftung steht verborgen um 10 px versetzt) und unabhängig davon, wie weit die
 * Seite gerade gescrollt ist.
 */
function boxIn(el: HTMLElement, sticky: HTMLElement): Box | null {
  if (!el.offsetWidth || !el.offsetHeight) return null;
  let x = 0;
  let y = 0;
  let node: HTMLElement | null = el;
  while (node && node !== sticky) {
    x += node.offsetLeft;
    y += node.offsetTop;
    node = node.offsetParent as HTMLElement | null;
  }
  return { l: x, t: y, r: x + el.offsetWidth, b: y + el.offsetHeight };
}

// Die letzte Rechnung: Bei gleichen Maßen (etwa mehrere Größenänderungen ohne echte Änderung,
// oder load nach dem Einflug) rechnet der Solver nicht noch einmal.
let solveKey = "";
let solveOut: ReturnType<typeof solveStage> | null = null;

/** Unterer sicherer Rand des Geräts (Home-Leiste), gemessen an einem unsichtbaren Fühler. */
let safeProbe: HTMLElement | null = null;
function safeBottom(): number {
  if (!safeProbe) {
    safeProbe = document.createElement("div");
    safeProbe.setAttribute("aria-hidden", "true");
    safeProbe.style.cssText =
      "position:fixed;left:0;bottom:0;width:0;height:env(safe-area-inset-bottom,0px);visibility:hidden;pointer-events:none";
    document.body.append(safeProbe);
  }
  return safeProbe.offsetHeight;
}

/**
 * Rückt den Fächer in den Raum und verkleinert ihn, bis alle sichtbaren Scheiben hineinpassen.
 * Rechts darf der Raum über den Rand reichen (die hinteren Scheiben laufen dort aus); die Front
 * selbst bleibt ganz im Bild (frontRight).
 */
function fitFan(
  G: Params,
  room: Box,
  frontRight: number,
  stack: boolean,
  Ws: number,
  H: number,
  vw: number,
  vh: number,
  shown: Profile,
) {
  const view = { vw, vh, Ws, H, P: perspectiveOf(Ws) };
  const visible = [0, 1, 2, 3].filter((d) => profile(shown, d) > 0.01);
  const T = slabT(Ws);
  for (let k = 0; k < 90; k++) {
    const rects = visible.map((d) =>
      stack ? rectWith(view, G, stackAt(d, Ws, H)) : stageRect(view, G, d),
    );
    const l = Math.min(...rects.map((v) => v.l));
    const r = Math.max(...rects.map((v) => v.r));
    const t = Math.min(...rects.map((v) => v.t));
    const b = Math.max(...rects.map((v) => v.b));
    const e = T * G.S;
    if (
      r - l + 2 * e > room.r - room.l ||
      b - t + 2 * e > room.b - room.t ||
      rects[0].r - l + e > frontRight - room.l
    ) {
      G.S *= 0.985;
      continue;
    }
    let dx = (room.l + room.r - l - r) / 2;
    // Die Front bleibt im Bild: dann rückt der ganze Fächer nach links, nie unter den Text.
    if (rects[0].r + dx > frontRight) dx = frontRight - rects[0].r;
    if (l + dx < room.l) dx = room.l - l;
    const dy = (room.t + room.b - t - b) / 2;
    if (Math.abs(dx) + Math.abs(dy) < 0.5) break;
    G.cx += dx;
    G.cy += dy;
  }
}

export function computeLayout(ctx: Ctx, cls: Cls): Layout {
  const { refs, s } = ctx;
  const { vw, vh } = s;
  const aspect = aspectOf(refs.scene);
  const spec = SPEC[cls];
  const side = SIDE.has(cls);
  const below = BELOW.has(cls);
  const ctlUnder = CTL_UNDER.has(cls);
  const stack = cls === "F";
  s.navBottom = refs.nav ? refs.nav.getBoundingClientRect().bottom : 0;
  // Freier Rand unten wie im CSS der Steuerung: im Hochformat max(3svh, sicherer Rand + 12px).
  const safe = safeBottom();
  const bottom = below ? Math.max(0.03 * vh, safe + 12) : safe + 14;
  const ctlH = refs.controls.offsetHeight || 44;
  // Höchstbreite der Steuerung wie im CSS (24rem); ihre gemessene Breite hinge an der Front
  // der letzten Messung und könnte nach einer Vergrößerung nicht mehr wachsen.
  const ctlW = 24 * (parseFloat(getComputedStyle(document.documentElement).fontSize) || 16);
  // Die Kästen der Beschriftung: Kopf mit Zählwerk und die freigehaltene Fläche des Textes.
  const caps: Box[] = [];
  let belowNeed = 0;
  if (below) {
    // Hochformat: Beschriftung und Steuerung stehen unter der Front; gebraucht wird ihre Höhe.
    const capStyle = getComputedStyle(refs.cap);
    const gap = parseFloat(capStyle.rowGap) || 12;
    belowNeed =
      refs.capHead.offsetHeight +
      gap +
      refs.layersBox.offsetHeight +
      18 +
      ctlH;
  } else {
    for (const el of [refs.capHead, refs.layersBox]) {
      const box = boxIn(el, refs.sticky);
      if (box) caps.push(box);
    }
  }
  const stagePref = params({ ...FLAT, ...spec.stage });
  // Kleinster Abstand der Fenster zur Beschriftung (HUM-413): max(24 px, 1,8 % der Breite).
  const gutter = Math.max(24, vw * 0.018);
  const solveIn = {
    vw,
    vh,
    aspect,
    Ws0: spec.ws(vw, vh, aspect),
    minWs: spec.ws(vw, vh, aspect) * 0.4,
    stage: stagePref,
    phisMin: spec.phisMin,
    betaMax: spec.betaMax,
    edge: EDGE[cls],
    navBottom: s.navBottom,
    caps,
    // Auch die Beschriftung unter der Bühne hält mindestens den Abstand der seitlichen.
    below: below ? { need: belowNeed, gap: Math.max(gutter, clamp(vh * 0.03, 16, 28)) } : null,
    ctl: ctlUnder ? { h: ctlH, w: ctlW, gap: ctlGapOf(vh) } : null,
    bottom,
    gutter,
    stagePos: spec.stagePos,
    stageNeg: STAGE_NEG,
    side: cls === "F" ? 10 : Math.max(10, vw * 0.02),
  };
  const key = JSON.stringify({ ...solveIn, cls });
  if (key !== solveKey || !solveOut) {
    solveKey = key;
    solveOut = solveStage(solveIn);
  }
  const solved = solveOut;
  const Ws = solved.Ws;
  const H = heightOf(Ws, aspect);
  const hero = params(spec.hero);
  // Der Raum des Fächers: rechts vom Text (side), sonst unter den Knöpfen, im Hochformat am
  // Telefon zwischen Lede und Knöpfen. Der Fächer steht mittig darin und wird kleiner, bis er
  // hineinpasst; im Breitbild bestimmt die Front die Größe, die hinteren Scheiben dürfen rechts
  // über den Rand laufen.
  const g = clamp(vh * 0.036, 20, 56);
  // Unter den Knöpfen steht der Hinweis zum Karussell, wenn er Platz hat; der Fächer beginnt
  // darunter.
  const hint = refs.hint && refs.hint.offsetHeight > 0 ? refs.hint : null;
  const heroBottom = Math.max(
    offsetIn(refs.actions, refs.sticky) + refs.actions.offsetHeight,
    hint ? offsetIn(hint, refs.sticky) + hint.offsetHeight : 0,
  );
  const textRight = refs.heroText.getBoundingClientRect().right;
  const mx = Math.max(10, 0.03 * vw);
  let room: Box;
  let frontRight = vw - mx;
  if (stack) {
    // Telefon hoch: die Front so breit wie möglich, die Karten dahinter laufen rechts über den
    // Rand aus (HUM-413).
    frontRight = vw - 10;
    room = {
      l: 10,
      r: vw + vw * 0.12,
      t: offsetIn(refs.heroText, refs.sticky) + refs.heroText.offsetHeight + 6,
      b: offsetIn(refs.actions, refs.sticky) - 6,
    };
  } else if (side) {
    const gapText = Math.max(28, vw * 0.035);
    room = {
      l: textRight + gapText,
      r: vw + vw * 0.08,
      t: s.navBottom + 12,
      b: vh - 0.6 * g,
    };
    // Die Front füllt die rechte Hälfte; rechts von ihr bleibt Platz, damit die Staffel
    // dahinter zu sehen ist und erst dann über den Rand läuft (Inhaber, 2026-11-02).
    frontRight = vw - Math.max(24, vw * 0.11);
  } else {
    room = { l: mx, r: vw - mx, t: heroBottom + 10, b: vh - 0.6 * g };
  }
  // Der Fächer rechnet mit der Fensterbreite der Bühne und einem eigenen Maßstab.
  hero.S = clamp((room.b - room.t) / H, 0.2, 1.6);
  fitFan(hero, room, frontRight, stack, Ws, H, vw, vh, spec.heroProfile);
  return {
    cls,
    Ws,
    H,
    T: slabT(Ws),
    P: perspectiveOf(Ws),
    hero,
    stage: solved.stage,
    morph: spec.morph,
    heroProfile: spec.heroProfile,
    heroNeg: HERO_NEG,
    stagePos: spec.stagePos,
    stageNeg: STAGE_NEG,
    stack,
    side,
    below,
    ctlUnder,
    capGap: solved.gap,
    ctl: solved.ctl,
    capTop: solved.capTop,
  };
}

/**
 * Die Höhe, in der die Bühne rechnet: die des angehefteten Abschnitts (100svh), nicht
 * innerHeight. Auf Telefonen ändert innerHeight sich, sobald die Adressleiste ein- oder
 * ausfährt; die Bühne baute sich dann mitten im Morph neu auf (HUM-412).
 */
export const stableHeight = (sticky: HTMLElement) =>
  sticky.clientHeight || window.innerHeight;

/**
 * Die Klasse des Bildausschnitts (HUM-418). /stage-class.js hat sie vor dem ersten Bild an
 * <html> gesetzt (data-cls, die Breite dazu in data-cls-w, dazu die Höhenmarken data-h<N>), und
 * das CSS legt danach Text und Stapel fest. Die Bühne übernimmt sie und teilt nur neu ein, wenn
 * sich die Breite seitdem geändert hat, auch beim Drehen (nextClass). Eine Änderung nur der Höhe
 * ist am Telefon die Adressleiste; neu eingeteilt sprängen Text und Stapel in eine andere Klasse.
 */
export function classOf(vw: number, vh: number): Cls {
  const root = document.documentElement;
  const prev = root.dataset.cls ?? "";
  const cls = nextClass(prev, Number(root.dataset.clsW), vw, vh);
  if (cls !== prev || root.dataset.clsW !== String(vw)) {
    root.dataset.cls = cls;
    root.dataset.clsW = String(vw);
    for (const b of H_BOUNDS) root.toggleAttribute(`data-h${b}`, heightMarks(vh).includes(b));
  }
  return cls;
}

/**
 * Misst den Bildausschnitt neu und schreibt die Maße, die das CSS braucht. reserve hält vorher
 * die Höhe der längsten Beschriftung frei, weil die Bühne um sie herum rechnet.
 */
export function measure(ctx: Ctx, reserve: () => void) {
  const { refs, s } = ctx;
  const { scene } = refs;
  s.vw = window.innerWidth;
  s.vh = stableHeight(refs.sticky);
  const cls = classOf(s.vw, s.vh);
  reserve();
  const lay = computeLayout(ctx, cls);
  s.lay = lay;
  scene.style.setProperty("--w", `${lay.Ws.toFixed(1)}px`);
  scene.style.setProperty("--t", `${lay.T.toFixed(2)}px`);
  refs.world.style.perspective = `${Math.round(lay.P)}px`;
  const R = lay.stage.r * lay.Ws;
  refs.ring.style.setProperty("--ring", `${(2 * (R + 0.06 * lay.Ws)).toFixed(0)}px`);
  refs.ring.style.setProperty("--ring-r", `${R.toFixed(0)}px`);
  // Der Ring läuft vor der Beschriftung aus: seine Vorderkante schnitte sonst als Strich durch
  // Überschrift und Text. Die Kante der Beschriftung in Koordinaten des Rings, als Anteil.
  const capL = refs.cap.getBoundingClientRect().left;
  const D = 2 * (R + 0.06 * lay.Ws);
  const cosB = Math.cos((lay.stage.beta * Math.PI) / 180);
  const end =
    lay.below || capL <= 0
      ? 0.96
      : clamp((D / 2 + (capL - 40 - s.vw / 2 - lay.stage.cx) / cosB) / D, 0.3, 0.96);
  refs.ring.style.setProperty("--ring-end", `${(end * 100).toFixed(1)}%`);
  const total = s.reduced ? 1.6 : 1 + lay.morph + 0.08;
  s.sceneH = Math.round(total * s.vh);
  scene.style.setProperty("--scene-h", `${s.sceneH}px`);
  s.sceneTop = scene.getBoundingClientRect().top + window.scrollY;
  // Pixel pro Fenster entlang des Bogens: projizierter Abstand von Front und d = +1.
  const view = { vw: s.vw, vh: s.vh, Ws: lay.Ws, H: lay.H, P: lay.P };
  const a = stageRect(view, lay.stage, 0);
  const b = stageRect(view, lay.stage, 1);
  s.pxPerStep = clamp(
    Math.hypot((b.l + b.r - a.l - a.r) / 2, (b.t + b.b - a.t - a.b) / 2),
    120,
    700,
  );
  if (s.lite) scene.dataset.lite = "";
  // Steuerung mittig unter der Front, Beschriftung unter der Bühne: die Lage aus der Rechnung.
  const c = refs.controls.style;
  if (lay.ctl) {
    c.setProperty("--ctl-x", `${lay.ctl.x}px`);
    c.setProperty("--ctl-y", `${lay.ctl.y}px`);
    c.setProperty("--ctl-w", `${lay.ctl.w}px`);
  } else for (const k of ["--ctl-x", "--ctl-y", "--ctl-w"]) c.removeProperty(k);
  if (lay.capTop !== null) scene.style.setProperty("--cap-t", `${lay.capTop}px`);
  else scene.style.removeProperty("--cap-t");
}
