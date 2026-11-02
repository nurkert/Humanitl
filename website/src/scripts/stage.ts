// Die Bühne der Startseite (HUM-334). Ein angehefteter Abschnitt: Der Morph beginnt mit
// dem ersten Pixel Scrollweg, ohne Haltestrecke. Die Fenster wandern vom Fächer auf
// einen Bogen, der nach rechts oben steigt; rechts unten entsteht Platz für die große
// Beschriftung.
//
// Der Fächer (HUM-348) zeigt nur drei bis vier Scheiben, seitlich geneigt und nach hinten
// gestaffelt, damit man ihre Dicke sieht. Die übrigen Fenster stehen unsichtbar auf ihren
// Plätzen und blenden im Morph auf, so kommen alle sieben auf dem Ring an.
//
// Danach scrollt die Seite normal weiter, kein Skript bewegt sie. Das Karussell dreht
// über Pfeile, Ziehen, Wischen, waagrechtes Scrollen und die Pfeiltasten, ohne Ende in beide
// Richtungen, und von selbst alle fünf Sekunden, solange niemand mit ihm arbeitet (HUM-389).
// Auf dem Bogen schauen alle Fenster gerade zum Betrachter, ihre Oberkante bleibt waagrecht;
// nur der Weg steigt nach rechts.
//
// Die Geometrie hängt an einem Parametersatz G (Größe, Lage, Kippung, Rolle des Bogens,
// Winkelabstand, Radius). Der Morph mischt Parameter, nicht Posen: so bleibt die
// Reihenfolge der Fenster in jedem Zwischenzustand erhalten. Pro Bild ändern sich nur
// transform, opacity und Custom Properties, die Deckkraft und Verschiebung von
// Lichtebenen steuern. Ein einziger rAF-Loop läuft nur, solange sich etwas bewegt.

import { type Follow, followStep } from "./stage/follow.mjs";
import {
  createDirectionLatch,
  createWheelAxis,
  WHEEL_IDLE_MS,
  wheelPixels,
} from "./stage/gesture.mjs";
import { frontVideo } from "./stage/video";

type Cls = "A" | "B" | "C" | "D" | "E" | "F" | "G";

/** Parametersatz der Bühne; Längen in px, Winkel in Grad, r in Fensterbreiten. */
interface Params {
  S: number;
  cx: number;
  cy: number;
  tilt: number;
  beta: number;
  rho: number;
  phis: number;
  r: number;
  kappa: number;
  delta: number;
  yaw: number;
  /** Wie weit das Fenster im Fokus vor den Bogen tritt, in Fensterbreiten (HUM-389). */
  lift: number;
}

/** Pose eines Fensters in Rig-Koordinaten. rx kippt um die Unterkante (Einflug). */
interface Pose {
  x: number;
  y: number;
  z: number;
  rz: number;
  ry: number;
  rx: number;
}

type Profile = [number, number][];

interface Layout {
  cls: Cls;
  Ws: number;
  Wh: number;
  H: number;
  T: number;
  P: number;
  hero: Params;
  stage: Params;
  hold: number;
  morph: number;
  heroProfile: Profile;
  heroNeg: Profile;
  stagePos: Profile;
  stageNeg: Profile;
  stack: boolean;
}

type Vec = [number, number, number];

// Seitenverhältnis der Aufnahmen, aus dem importierten Bild (data-aspect an der Szene).
const ASPECT =
  Number(document.querySelector<HTMLElement>("[data-scene]")?.dataset.aspect) ||
  2560 / 1352;
// Breite des Glasrahmens in Fensterbreiten, wie --pad in Home.astro.
const PAD = 0.005;
/** Höhe eines Fensters: die Aufnahme mit ihrem Verhältnis plus der Rahmen oben und unten. */
const heightOf = (Ws: number) => (Ws * (1 - 2 * PAD)) / ASPECT + 2 * PAD * Ws;
const RAD = Math.PI / 180;

// ---- Kleine Mathematik ------------------------------------------------------------

const clamp = (v: number, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const map = (v: number, a: number, b: number) => clamp((v - a) / (b - a));
const smoothstep = (t: number) => t * t * (3 - 2 * t);
const smootherstep = (t: number) => t * t * t * (t * (t * 6 - 15) + 10);
const easeInOutCubic = (t: number) =>
  t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
const easeInOutSine = (t: number) => -(Math.cos(Math.PI * t) - 1) / 2;
const easeOutCubic = (t: number) => 1 - Math.pow(1 - t, 3);
const easeOutQuad = (t: number) => 1 - (1 - t) * (1 - t);
const easeInCubic = (t: number) => t * t * t;

/** Stückweise lineare Kurve über |d|; nach dem letzten Punkt gilt dessen Wert. */
function profile(points: Profile, x: number): number {
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

/** Kubische Bézierkurve wie in CSS, gelöst mit wenigen Newton-Schritten. */
function bezier(x1: number, y1: number, x2: number, y2: number) {
  const cx = 3 * x1;
  const bx = 3 * (x2 - x1) - cx;
  const ax = 1 - cx - bx;
  const cy = 3 * y1;
  const by = 3 * (y2 - y1) - cy;
  const ay = 1 - cy - by;
  const x = (t: number) => ((ax * t + bx) * t + cx) * t;
  const dx = (t: number) => (3 * ax * t + 2 * bx) * t + cx;
  return (v: number) => {
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

/** Gedämpfte Feder in geschlossener Form, 0 bis 1; deterministisch über die Zeit in s. */
function springU(t: number, w: number, z: number): number {
  if (t <= 0) return 0;
  if (z >= 1) return 1 - Math.exp(-w * t) * (1 + w * t);
  const wd = w * Math.sqrt(1 - z * z);
  return (
    1 -
    Math.exp(-z * w * t) *
      (Math.cos(wd * t) + ((z * w) / wd) * Math.sin(wd * t))
  );
}

// Drehungen wie die CSS-Funktionen rotateX/Y/Z (y zeigt nach unten, z zum Betrachter).
function rotX([x, y, z]: Vec, a: number): Vec {
  const c = Math.cos(a * RAD);
  const s = Math.sin(a * RAD);
  return [x, y * c - z * s, y * s + z * c];
}
function rotY([x, y, z]: Vec, a: number): Vec {
  const c = Math.cos(a * RAD);
  const s = Math.sin(a * RAD);
  return [x * c + z * s, y, -x * s + z * c];
}
function rotZ([x, y, z]: Vec, a: number): Vec {
  const c = Math.cos(a * RAD);
  const s = Math.sin(a * RAD);
  return [x * c - y * s, x * s + y * c, z];
}
const norm = ([x, y, z]: Vec): Vec => {
  const l = Math.hypot(x, y, z) || 1;
  return [x / l, y / l, z / l];
};
const dot = (a: Vec, b: Vec) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

// Bis hierher reicht der Nebel im Morph (HUM-341).
const FOG_END = 0.3;

// Weltlicht: oben vorn leicht links, fest im Raum.
const LIGHT = norm([-0.3, -0.85, 0.45]);
const LIGHT_FLAT = norm([LIGHT[0], 0, LIGHT[2]]);
// Glanzstellung für den Rim und den Glint: knapp links oberhalb der Blickachse, damit die
// Front im Ruhezustand glänzt und jedes Fenster beim Drehen durch die Glanzstellung läuft.
// Der Halbvektor aus LIGHT läge 30 Grad über der Achse und träfe aufrechte Fenster nie.
const SPEC_H = norm([-0.1, -0.3, 1]);

// ---- Bildausschnitt je Seitenverhältnis -------------------------------------------

function classify(vw: number, vh: number): Cls {
  const a = vw / vh;
  if (vh < 500 && a > 1.6) return "G";
  if (vw < 700 && a < 0.75) return "F";
  if (a >= 2) return "A";
  if (a > 1.6) return "B";
  if (a >= 1.45) return "C";
  if (a >= 1.15) return "D";
  if (vw >= 700) return "E";
  return "F";
}

const HERO_FAN: Profile = [
  [1, 1],
  [2, 0.8],
  [2.6, 0],
];
// Links der Front liegt im Fächer nichts: dort warten die Fenster unsichtbar auf den Morph.
const HERO_NEG: Profile = [
  [0, 1],
  [0.6, 0],
];
const STAGE_POS: Profile = [
  [2, 1],
  [3, 0.5],
  [3.4, 0],
];
// Links der Front steht auf der Bühne nichts mehr (HUM-389): das gehende Fenster blendet auf
// dem Weg nach d = -1 aus, links unter der Front liegen die Pfeile.
const STAGE_NEG: Profile = [
  [0, 1],
  [0.45, 0.8],
  [0.95, 0],
];
const PHONE_POS: Profile = [
  [2, 1],
  [2.6, 0],
];

// Mindeststeigung des Bogens je Klasse (spezifizierte Werte); weniger nimmt die
// Prüfung im Bildausschnitt nie zurück.
const BETA_MIN: Record<Cls, number> = {
  A: -9,
  B: -10,
  C: -10,
  D: -9,
  E: -11,
  F: -5,
  G: -6,
};

// Neigung des Telefon-Stapels in Grad: dieselbe Blickrichtung wie der Fächer.
const STACK_YAW = -28;

// Wo die Front auf der Bühne steht: linker Rand als Anteil der Breite, null heißt mittig.
const EDGE: Record<Cls, number | null> = {
  A: 0.04,
  B: 0.035,
  C: 0.025,
  D: 0.02,
  E: null,
  F: null,
  G: 0.015,
};
// d = +1 darf um diesen Anteil der Leiste unter die Navigation gleiten: unscharf und
// abgedunkelt stört es dort nicht, und die Front gewinnt die Höhe.
const TUCK = 0.5;
// Rand um die Beschriftung gegen d = +1: negativ, die Ecke darf unter den Kasten reichen.
const CORNER = -12;

// Bildausschnitte, in denen die Steuerung unter der Front steht statt an der Beschriftung.
const CTL_FREE = new Set<Cls>(["A", "B", "C", "D"]);
const ctlGapOf = (vh: number) => clamp(vh * 0.022, 14, 28);

// Perspektive der Bühne; dieselbe Tiefe wirkt stärker, die Front löst sich vom Bogen.
const perspectiveOf = (Ws: number) => Math.max(1200, 2.4 * Ws);

// Die Dicke muss im Fächer als Seitenwand zu lesen sein, auch verkleinert und geneigt.
const slabT = (Ws: number) => clamp(0.03 * Ws, 10, 30);

interface Box {
  l: number;
  t: number;
  r: number;
  b: number;
}

/** Anteil der Fläche von r, der im Kasten c (mit 8 px Rand) liegt. */
function overlap(c: DOMRect, r: Box): number {
  const w = Math.min(c.right + 8, r.r) - Math.max(c.left - 8, r.l);
  const h = Math.min(c.bottom + 8, r.b) - Math.max(c.top - 8, r.t);
  if (w <= 0 || h <= 0) return 0;
  return (w * h) / Math.max(1, (r.r - r.l) * (r.b - r.t));
}

function params(p: Partial<Params>): Params {
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
    ...p,
  };
}

// Der Fächer der breiten Klassen: Scheiben fast parallel, jede vier Grad weiter gedreht.
const FAN: Partial<Params> = {
  tilt: 4,
  phis: 16,
  r: 1.3,
  kappa: 0.25,
  delta: 0.22,
  yaw: -32,
};

// Die Bühne (HUM-389): jedes Fenster frontal und waagrecht. kappa und yaw drehen nicht mehr,
// rho = 0 hebt die Rolle des Bogens am Fenster auf. Das Fenster im Fokus tritt um LIFT vor:
// es wird größer, und beim Wechsel liegt das ankommende Fenster immer vor dem gehenden.
const LIFT = 0.16;
const FLAT: Partial<Params> = { kappa: 0, yaw: 0, rho: 0, lift: LIFT };

/**
 * Der Stapel am Telefon: die Karten rechts der Front staffeln sich schräg nach hinten, die
 * unsichtbaren links liegen dicht hinter ihr und öffnen sich erst im Morph.
 */
function stackAt(d: number, Ws: number, H: number): Pose {
  const s = d >= 0 ? d : -0.6 * d;
  return {
    x: Math.sign(d) * 0.13 * Ws * s,
    y: -0.03 * H * s,
    z: -0.24 * Ws * s,
    rz: 0,
    ry: STACK_YAW,
    rx: 0,
  };
}

// ---- Die Bühne --------------------------------------------------------------------

function init(scene: HTMLElement) {
  const sticky = scene.querySelector<HTMLElement>("[data-sticky]")!;
  const world = scene.querySelector<HTMLElement>("[data-world]")!;
  const rig = scene.querySelector<HTMLElement>("[data-rig]")!;
  const ring = scene.querySelector<HTMLElement>("[data-ring]")!;
  const pool = scene.querySelector<HTMLElement>("[data-pool]")!;
  const wins = [...scene.querySelectorAll<HTMLElement>("[data-win]")];
  const heroText = scene.querySelector<HTMLElement>("[data-hero]")!;
  const actions = scene.querySelector<HTMLElement>("[data-hero-actions]")!;
  const spot = scene.querySelector<HTMLElement>("[data-spot]")!;
  // Nebel (HUM-341): Tiefe je Ebene für die Parallaxe, vorn bewegt sich mehr.
  const fog = scene.querySelector<HTMLElement>("[data-fog]")!;
  const fogDepth: Record<string, number> = { far: 0.25, rays: 0.6, near: 0.9 };
  const fogLayers = [...fog.querySelectorAll<HTMLElement>("[data-fog-layer]")];
  let fogGone = false;
  const hit = scene.querySelector<HTMLElement>("[data-hit]")!;
  const deck = scene.querySelector<HTMLElement>("[data-deck]")!;
  const cap = scene.querySelector<HTMLElement>("[data-cap]")!;
  const capHead = scene.querySelector<HTMLElement>("[data-cap-head]")!;
  // Ab hier sind Pfeile sichtbar und Drehen, Ziehen und Tastatur zugleich möglich (HUM-347).
  const LIVE_AT = 0.85;
  const controls = scene.querySelector<HTMLElement>("[data-controls]")!;
  // Je Modul ein Abschnitt; der aktive füllt sich im Selbstlauf (HUM-389).
  const segs = [...scene.querySelectorAll<HTMLElement>("[data-seg]")];
  const odo = scene.querySelector<HTMLElement>("[data-odo]")!;
  const live = scene.querySelector<HTMLElement>("[data-live-text]")!;
  const layerEls = [...scene.querySelectorAll<HTMLElement>("[data-layer]")];
  const nav = document.querySelector<HTMLElement>(
    "[data-nav] .nav__bar, [data-nav]",
  );
  const frontImg = wins[0]?.querySelector("img");
  // Die Unschärfe: je Fenster die weiche Ebene und ihr Saum.
  const dofs = wins.map((w) => w.querySelector<HTMLElement>("[data-dof]")!);
  const feathers = wins.map((w) =>
    w.querySelector<HTMLElement>("[data-feather]")!,
  );
  const n = wins.length;
  // Kann sich zur Laufzeit ändern; dann halten Autoplay und Video sofort an (unten).
  const reducedQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
  let reduced = reducedQuery.matches;
  // Der Ablauf im vorderen Fenster (HUM-389) und sein Fenster.
  const videoEl = scene.querySelector<HTMLVideoElement>("[data-video]");
  const videoWin = videoEl ? wins.findIndex((w) => w.contains(videoEl)) : -1;
  const video = frontVideo(videoEl, reduced);
  const dev = import.meta.env.DEV;

  // Abstand auf dem Ring, gefaltet in [-n/2, n/2): jedes Fenster hat links und rechts Nachbarn.
  const wrap = (d: number) => ((((d + n / 2) % n) + n) % n) - n / 2;
  const mod = (v: number) => ((v % n) + n) % n;

  let vw = 0;
  let vh = 0;
  let lay!: Layout;
  let sceneTop = 0;
  let sceneH = 0;
  let pxPerStep = 400;
  let navBottom = 0;

  // Zustand: Morph (roh aus dem Scrollweg und geglättet, mit Geschwindigkeit), Ringposition,
  // Einflug.
  let mRaw = 0;
  let m = 0;
  const follow: Follow = { m: 0, v: 0 };
  let pos = 0;
  let vel = 0;
  let target = 0;
  let springOn = false;
  let wheelTimer = 0;
  // Ein Wechsel per Pfeil, Taste oder Selbstlauf ist eine einzige Kurve für alle Fenster. Ein
  // weiterer Klick mitten in der Fahrt hängt eine zweite Kurve an, die sich zur ersten addiert:
  // so ändert sich das Ziel ohne Sprung in Lage oder Richtung (HUM-389).
  let tweens: { t0: number; delta: number }[] = [];
  // Seit dem ersten Klick liefen mehrere Kurven zugleich; erst wenn alle stehen, wechselt die
  // Beschriftung (render).
  let multiTurn = false;
  let tweenBase = 0;
  // Ziehen oder waagrechtes Scrollen: der Ring folgt der Hand, steerDir ist ihre Richtung. Sie
  // kippt erst nach einer Gegenbewegung von 0,06 Fenstern, sonst wechselte der Vortritt beim
  // Zittern der Hand im Takt der Ereignisse (stage/gesture.mjs).
  let steering = false;
  const steerLatch = createDirectionLatch();
  let steerDir = 1;
  // Vortritt je Fenster, 0 bis 1: folgt schnell dem Fenster, das zum Fokus wird.
  const liftW: number[] = [];
  let introT0 = -1;
  let introDone = reduced;
  let introStarts: Pose[] = [];
  let introDelay: number[] = [];
  let introOrder: number[] = [];
  let lastNow = 0;
  let frame = 0;
  let active = 0;
  let liveState = false;
  let lite = (navigator.hardwareConcurrency ?? 8) <= 4;
  let introFrames = 0;
  let introFrameSum = 0;

  // Schreibt Stil nur, wenn sich der Wert geändert hat.
  const cache = new WeakMap<HTMLElement, Record<string, string>>();
  function put(el: HTMLElement, prop: string, value: string) {
    let c = cache.get(el);
    if (!c) cache.set(el, (c = {}));
    if (c[prop] === value) return;
    c[prop] = value;
    if (prop.startsWith("--")) el.style.setProperty(prop, value);
    else (el.style as unknown as Record<string, string>)[prop] = value;
  }

  // ---- Messen ---------------------------------------------------------------------

  function offsetIn(el: HTMLElement): number {
    let y = 0;
    let node: HTMLElement | null = el;
    while (node && node !== sticky) {
      y += node.offsetTop;
      node = node.offsetParent as HTMLElement | null;
    }
    return y;
  }

  function layout(cls: Cls): Layout {
    const g = clamp(vh * 0.036, 20, 56);
    const heroBottom = offsetIn(actions) + actions.offsetHeight;
    const fanRoom = (k: number) => (vh - heroBottom - k * g) / 0.66;
    let Ws = 0;
    let Wh = 0;
    let hero: Params;
    let stage: Params;
    let heroProfile = HERO_FAN;
    let stagePos = STAGE_POS;
    // Keine Haltestrecke: Der Morph beginnt mit dem ersten Pixel Scrollweg, damit die
    // Bühne nie stehen bleibt, während die Seite sich schon bewegt (Inhaber, 2026-10-30).
    let hold = 0;
    let morph = 1;
    let stack = false;
    switch (cls) {
      case "A":
        Wh = Math.min(vw * 0.36, vh * 0.8);
        Ws = Math.min(vw * 0.52, vh * 1.2);
        hero = params({
          tilt: 4,
          phis: 15,
          r: 1.3,
          kappa: 0.27,
          delta: 0.22,
          yaw: -32,
        });
        stage = params({
          ...FLAT,
          cy: -vh * 0.06,
          tilt: 3,
          beta: -14,
          phis: 32,
          r: 1.2,
          delta: 0.05,
        });
        // Der breite Schirm hat Platz für eine vierte Scheibe.
        heroProfile = [
          [1, 1],
          [2, 0.75],
          [3, 0.45],
          [3.4, 0],
        ];
        stagePos = PHONE_POS;
        break;
      case "B":
        Wh = Math.min(vw * 0.42, fanRoom(1));
        Ws = Math.min(vw * 0.58, vh * 1.05);
        hero = params(FAN);
        stage = params({
          ...FLAT,
          cy: -vh * 0.1,
          tilt: 2,
          beta: -15,
          phis: 35,
          r: 1.08,
          delta: 0.05,
        });
        stagePos = PHONE_POS;
        break;
      case "C":
        Wh = Math.min(vw * 0.46, fanRoom(1));
        Ws = Math.min(vw * 0.58, vh * 0.98);
        hero = params(FAN);
        stage = params({
          ...FLAT,
          cy: -vh * 0.12,
          tilt: 2,
          beta: -15,
          phis: 35,
          r: 1.08,
          delta: 0.05,
        });
        stagePos = PHONE_POS;
        break;
      case "D":
        Wh = Math.min(vw * 0.52, fanRoom(1));
        Ws = vw * 0.66;
        hero = params({ ...FAN, phis: 14 });
        stage = params({
          ...FLAT,
          cy: -vh * 0.15,
          tilt: 3,
          beta: -13,
          phis: 35,
          r: 1.05,
          delta: 0.05,
        });
        // Bei 30° Winkelabstand stünde d = 3 schon hinter dem Scheitel des Bogens.
        stagePos = PHONE_POS;
        break;
      case "E":
        Wh = vw * 0.76;
        Ws = vw * 0.9;
        hero = params({ ...FAN, phis: 10, delta: 0.26 });
        stage = params({
          ...FLAT,
          cy: -vh * 0.16,
          tilt: 3,
          beta: -14,
          phis: 34,
          r: 1.15,
          delta: 0.05,
        });
        stagePos = PHONE_POS;
        break;
      case "F":
        Wh = Math.min(vw * 0.86, vh * 0.3 * ASPECT);
        Ws = vw * 0.93;
        hero = params({ tilt: 4 });
        stage = params({
          ...FLAT,
          cy: -vh * 0.16,
          tilt: 3,
          beta: -5,
          phis: 17,
          r: 2.4,
          delta: 0.05,
        });
        heroProfile = [
          [2, 1],
          [2.6, 0],
        ];
        stagePos = PHONE_POS;
        morph = 0.8;
        stack = true;
        break;
      case "G":
      default:
        Wh = vw * 0.4;
        Ws = Math.min(vw * 0.54, vh * 0.7 * ASPECT);
        hero = params({ ...FAN, phis: 12, delta: 0.2, yaw: -30 });
        stage = params({
          ...FLAT,
          cy: vh * 0.06,
          tilt: 2,
          beta: -8,
          phis: 30,
          r: 1.2,
          delta: 0.05,
        });
        // Telefon quer: rechts steht die Beschriftung, die Nachbarn rechts treten zurück.
        stagePos = [
          [0, 1],
          [0.5, 1],
          [0.9, 0],
        ];
        hold = 0;
        morph = 0.65;
        break;
    }
    // Bildausschnitt der Bühne prüfen. Die Front steht fest am linken Rand ihrer Klasse
    // (Hochformat: mittig), alles Weitere läuft von dort. Sie bleibt mindestens 16 px unter
    // der Navigation, d = +1 darf um TUCK der Leiste darunter gleiten; die Beschriftung
    // schneidet die Front nicht, d = +1 höchstens mit der Ecke, d = +2 ragt höchstens 15 %
    // hinein. Erst nimmt der Bogen Steigung zurück bis zum Mindestwert der Klasse, dann
    // sinkt oder steigt die Bühne, und reicht der Platz dann nicht, wird sie kleiner.
    navBottom = nav ? nav.getBoundingClientRect().bottom : 0;
    const capBox = cap.getBoundingClientRect();
    const capOn = capBox.width > 0 && capBox.height > 0;
    // Gegen d = +1 zählt auf breiten Schirmen nur der Text: die Konturziffer darf über dem
    // unscharfen Nachbarn stehen, das staffelt die Tiefe.
    const headBottom = capHead.getBoundingClientRect().bottom;
    const textBox =
      cls === "A" || cls === "B" || cls === "C"
        ? new DOMRect(
            capBox.left,
            headBottom,
            capBox.width,
            capBox.bottom - headBottom,
          )
        : capBox;
    const betaMin = BETA_MIN[cls];
    // Auf breiten Schirmen stehen Pfeile und Abschnitte unter der linken Unterkante der Front
    // (HUM-389); sie brauchen ihre Höhe unter dem Fenster.
    const ctlFree = CTL_FREE.has(cls);
    const ctlGap = ctlGapOf(vh);
    const ctlH = controls.offsetHeight || 44;
    const o1 = profile(stagePos, 1);
    const o2 = profile(stagePos, 2);
    const edge = EDGE[cls];
    for (let k = 0; k < 60; k++) {
      const f = stageRect(stage, 0, Ws);
      stage.cx +=
        edge === null ? (vw - f.l - f.r) / 2 : Math.round(edge * vw) - f.l;
      const rects = [0, 1, 2].map((d) => stageRect(stage, d, Ws));
      const need = navBottom + 16;
      const top = Math.min(
        rects[0].t,
        o1 > 0.3 ? rects[1].t + TUCK * navBottom : Infinity,
      );
      const capHit =
        capOn &&
        (hitsQuad(capBox, rects[0].q) ||
          (o1 > 0.3 && hitsQuad(textBox, rects[1].q, CORNER)) ||
          (o2 > 0.3 && overlap(capBox, rects[2]) > 0.15));
      const low = ctlFree && rects[0].b + ctlGap + ctlH > vh - 14;
      if (top >= need && !capHit && !low) break;
      const high = top < need;
      if (high && stage.beta < betaMin - 0.01)
        stage.beta = Math.min(betaMin, stage.beta + 1);
      else if (high && !capHit && !low)
        stage.cy += Math.min(need - top, 0.02 * vh) + 1;
      else if ((capHit || low) && top > need + 2) {
        // Beschriftung oder Steuerung stoßen an: die Bühne steigt in den freien Raum unter
        // der Navigation.
        stage.cy -= Math.min(top - need, 0.02 * vh);
      } else Ws *= 0.985;
    }
    hero.S = Wh / Ws;
    const H = heightOf(Ws);
    const T = slabT(Ws);
    const P = perspectiveOf(Ws);
    const out: Layout = {
      cls,
      Ws,
      Wh,
      H,
      T,
      P,
      hero,
      stage,
      hold,
      morph,
      heroProfile,
      heroNeg: HERO_NEG,
      stagePos,
      stageNeg: STAGE_NEG,
      stack,
    };
    // Der Raum des Fächers: unter den Knöpfen oder rechts vom Text, im Hochformat zwischen
    // Lede und Knöpfen. Der Fächer steht mittig darin und wird kleiner, bis er hineinpasst.
    const side = cls === "A" || cls === "G";
    const textRight = heroText.getBoundingClientRect().right;
    const mx = Math.max(10, 0.03 * vw);
    const room: Box = stack
      ? {
          l: 8,
          r: vw - 8,
          t: offsetIn(heroText) + heroText.offsetHeight + 8,
          b: offsetIn(actions) - 8,
        }
      : {
          l: side ? textRight + 20 : mx,
          r: vw - mx,
          t: (side ? navBottom : heroBottom) + 10,
          b: vh - 0.6 * g,
        };
    fitFan(hero, room, stack, Ws, H, P, heroProfile);
    return out;
  }

  /** Rückt den Fächer mittig in den Raum und verkleinert ihn, bis er hineinpasst. */
  function fitFan(
    G: Params,
    room: Box,
    stack: boolean,
    Ws: number,
    H: number,
    P: number,
    shown: Profile,
  ) {
    const visible = [0, 1, 2, 3].filter((d) => profile(shown, d) > 0.01);
    const T = slabT(Ws);
    for (let k = 0; k < 80; k++) {
      const rects = visible.map((d) =>
        stack ? rectWith(G, stackAt(d, Ws, H), Ws, H, P) : stageRect(G, d, Ws),
      );
      const l = Math.min(...rects.map((v) => v.l));
      const r = Math.max(...rects.map((v) => v.r));
      const t = Math.min(...rects.map((v) => v.t));
      const b = Math.max(...rects.map((v) => v.b));
      // Die Dicke zählt mit: sie steht hinter der Vorderkante hervor.
      const e = T * G.S;
      if (r - l + 2 * e > room.r - room.l || b - t + 2 * e > room.b - room.t) {
        G.S *= 0.985;
        continue;
      }
      const dx = (room.l + room.r - l - r) / 2;
      const dy = (room.t + room.b - t - b) / 2;
      if (Math.abs(dx) + Math.abs(dy) < 0.5) break;
      G.cx += dx;
      G.cy += dy;
    }
  }

  /** Projizierte Ecken eines Fensters mit Pose p unter G, im Umlaufsinn. */
  function quadWith(
    G: Params,
    p: Pose,
    Ws: number,
    H: number,
    P: number,
  ): [number, number][] {
    return [
      [-1, -1],
      [1, -1],
      [1, 1],
      [-1, 1],
    ].map(([sx, sy]) => {
      let c: Vec = [(sx * Ws) / 2, (sy * H) / 2, 0];
      c = rotY(c, p.ry);
      c = rotZ(c, p.rz);
      const w = toWorld(G, [c[0] + p.x, c[1] + p.y, c[2] + p.z]);
      const k = P / Math.max(1, P - w[2]);
      return [vw / 2 + w[0] * k, vh / 2 + w[1] * k] as [number, number];
    });
  }

  function boxOf(q: [number, number][]): Box & { q: [number, number][] } {
    const xs = q.map((v) => v[0]);
    const ys = q.map((v) => v[1]);
    return {
      l: Math.min(...xs),
      t: Math.min(...ys),
      r: Math.max(...xs),
      b: Math.max(...ys),
      q,
    };
  }

  function rectWith(G: Params, p: Pose, Ws: number, H: number, P: number) {
    return boxOf(quadWith(G, p, Ws, H, P));
  }

  /** Trennachsentest: schneidet der Kasten c (mit Rand e) das konvexe Viereck q? */
  function hitsQuad(c: DOMRect, q: [number, number][], e = 16): boolean {
    const rect: [number, number][] = [
      [c.left - e, c.top - e],
      [c.right + e, c.top - e],
      [c.right + e, c.bottom + e],
      [c.left - e, c.bottom + e],
    ];
    const axes: [number, number][] = [
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
      if (
        Math.max(...pa) < Math.min(...pb) ||
        Math.max(...pb) < Math.min(...pa)
      )
        return false;
    }
    return true;
  }

  function stageRect(G: Params, d: number, Ws: number) {
    const phi = d * G.phis * RAD;
    const R = G.r * Ws;
    const p: Pose = {
      x: R * Math.sin(phi),
      y: 0,
      z:
        -R * (1 - Math.cos(phi)) -
        G.delta * Math.abs(d) * Ws +
        (d === 0 ? G.lift * Ws : 0),
      rz: -(1 - G.rho) * G.beta,
      ry: -G.kappa * d * G.phis + G.yaw,
      rx: 0,
    };
    return rectWith(G, p, Ws, heightOf(Ws), perspectiveOf(Ws));
  }

  // Die Beschriftung hält die Höhe der längsten Fassung frei, damit Index und Augenbraue
  // beim Wechsel nicht springen; gemessen an einer unsichtbaren Kopie einer Schicht.
  const layersBox = scene.querySelector<HTMLElement>(".cap__layers")!;
  let probe: HTMLElement | null = null;
  function reserveCaption() {
    if (!probe) {
      probe = layerEls[0].cloneNode(true) as HTMLElement;
      probe.removeAttribute("data-layer");
      probe.classList.add("cap__probe");
      layersBox.append(probe);
    }
    const fields = [
      "[data-cap-name]",
      "[data-cap-title]",
      "[data-cap-text]",
    ].map((q) => probe!.querySelector<HTMLElement>(q)!);
    layersBox.style.minHeight = "";
    let max = 0;
    for (const win of wins) {
      words(fields[0], win.dataset.name ?? "");
      words(fields[1], win.dataset.title ?? "");
      words(fields[2], win.dataset.text ?? "");
      max = Math.max(max, probe.offsetHeight);
    }
    layersBox.style.minHeight = `${Math.ceil(max)}px`;
  }

  /**
   * Die Höhe, in der die Bühne rechnet: die des angehefteten Abschnitts (100svh), nicht
   * innerHeight. Auf Telefonen ändert innerHeight sich, sobald die Adressleiste beim Scrollen
   * ein- oder ausfährt; die Bühne baute sich dann mitten im Morph neu auf und sprang, und ihre
   * Projektion rechnete mit einer anderen Mitte als die Perspektive der .world.
   */
  const stableHeight = () => sticky.clientHeight || window.innerHeight;

  function measure() {
    vw = window.innerWidth;
    vh = stableHeight();
    const cls = classify(vw, vh);
    scene.dataset.cls = cls;
    reserveCaption();
    lay = layout(cls);
    scene.style.setProperty("--w", `${lay.Ws.toFixed(1)}px`);
    odoEm = parseFloat(getComputedStyle(odoStrip).fontSize) || 0;
    scene.style.setProperty("--t", `${lay.T.toFixed(2)}px`);
    world.style.perspective = `${Math.round(lay.P)}px`;
    const D = 2 * (lay.stage.r * lay.Ws + 0.06 * lay.Ws);
    ring.style.setProperty("--ring", `${D.toFixed(0)}px`);
    ring.style.setProperty(
      "--ring-r",
      `${(lay.stage.r * lay.Ws).toFixed(0)}px`,
    );
    const total = reduced ? 1.6 : 1 + lay.hold + lay.morph + 0.08;
    scene.style.setProperty("--scene-h", `${Math.round(total * vh)}px`);
    sceneTop = scene.getBoundingClientRect().top + window.scrollY;
    sceneH = Math.round(total * vh);
    // Pixel pro Fenster entlang des Bogens: projizierter Abstand von Front und d = +1.
    const a = project(toWorld(lay.stage, [0, 0, 0]));
    const b = project(toWorld(lay.stage, slotVec(lay.stage, 1)));
    pxPerStep = clamp(Math.hypot(b[0] - a[0], b[1] - a[1]), 140, 700);
    if (lite) scene.dataset.lite = "";
    // Die Steuerung unter der linken Unterkante der Front, so breit wie diese (höchstens
    // 26 rem, das setzt das CSS). In den übrigen Bildausschnitten stellt das CSS sie hin.
    if (CTL_FREE.has(lay.cls)) {
      const f = stageRect(lay.stage, 0, lay.Ws);
      controls.style.setProperty("--ctl-x", `${Math.round(f.l)}px`);
      controls.style.setProperty(
        "--ctl-y",
        `${Math.round(f.b + ctlGapOf(vh))}px`,
      );
      controls.style.setProperty("--ctl-w", `${Math.round(f.r - f.l)}px`);
    } else {
      for (const k of ["--ctl-x", "--ctl-y", "--ctl-w"])
        controls.style.removeProperty(k);
    }
  }

  // ---- Geometrie ------------------------------------------------------------------

  /** Rig-Transform auf einen lokalen Punkt: scale, rotateZ(β), rotateX(tilt), translate. */
  function toWorld(G: Params, p: Vec): Vec {
    let v: Vec = [p[0] * G.S, p[1] * G.S, p[2] * G.S];
    v = rotZ(v, G.beta);
    v = rotX(v, G.tilt);
    return [v[0] + G.cx, v[1] + G.cy, v[2]];
  }

  /** Perspektive der .world: Ursprung in der Mitte des Viewports. */
  function project(v: Vec): [number, number] {
    const k = lay.P / Math.max(1, lay.P - v[2]);
    return [vw / 2 + v[0] * k, vh / 2 + v[1] * k];
  }

  /** Platz auf dem Bogen; lw ist der Anteil des Vortritts im Fokus (0 bis 1). */
  function slotVec(G: Params, d: number, lw = 0): Vec {
    const phi = d * G.phis * RAD;
    const R = G.r * lay.Ws;
    return [
      R * Math.sin(phi),
      0,
      -R * (1 - Math.cos(phi)) -
        G.delta * Math.abs(d) * lay.Ws +
        G.lift * lw * lay.Ws,
    ];
  }

  function slotPose(G: Params, d: number, rho: number, lw = 0): Pose {
    const [x, y, z] = slotVec(G, d, lw);
    return {
      x,
      y,
      z,
      rz: -(1 - rho) * G.beta,
      ry: -G.kappa * d * G.phis + G.yaw,
      rx: 0,
    };
  }

  /** Rang im Telefon-Stapel: d = +1 liegt direkt hinter der Front, d = -1 dahinter. */
  const rankOf = (d: number) => (d >= 0 ? (d <= 1 ? d : 2 * d - 1) : -2 * d);

  const stackPose = (d: number) => stackAt(d, lay.Ws, lay.H);

  /** Der Parametersatz für den Morph m, je Gruppe mit eigener Kurve. */
  function mixG(mm: number): Params {
    const h = lay.hero;
    const s = lay.stage;
    // S und die Lage sprechen sofort an, sobald der Text zu gehen beginnt.
    const eS = easeInOutSine(mm);
    const eArc = smootherstep(map(mm, 0.05, 0.9));
    const eC = easeInOutSine(map(mm, 0.04, 0.85));
    const eT = easeInOutSine(map(mm, 0, 0.85));
    const eB = smoothstep(map(mm, 0.35, 1));
    return {
      S: lerp(h.S, s.S, eS),
      cx: lerp(h.cx, s.cx, eC),
      cy: lerp(h.cy, s.cy, eC),
      tilt: lerp(h.tilt, s.tilt, eT),
      beta: lerp(h.beta, s.beta, eB),
      rho: s.rho,
      phis: lerp(h.phis, s.phis, eArc),
      r: lerp(h.r, s.r, eArc),
      kappa: lerp(h.kappa, s.kappa, eArc),
      delta: lerp(h.delta, s.delta, eArc),
      yaw: lerp(h.yaw, s.yaw, eArc),
      lift: lerp(h.lift, s.lift, eArc),
    };
  }

  /** Pose von Fenster i im Morph; ρ ist je Fenster gestaffelt, lw ist sein Vortritt. */
  function morphPose(G: Params, d: number, mm: number, lw = 0): Pose {
    const mi = clamp((mm - 0.04 * Math.abs(d)) / 0.88);
    const rho = lerp(lay.hero.rho, lay.stage.rho, smoothstep(mi));
    if (!lay.stack) return slotPose(G, d, rho, lw);
    // Telefon hoch: der Stapel fächert sich zum Bogen auf, Kanal für Kanal.
    const h = stackPose(d);
    const s = slotPose(lay.stage, d, rho, lw * smoothstep(mm));
    const rank = Math.min(3, rankOf(d));
    const e = clamp((mm - 0.08 * rank) / (1 - 0.24));
    const ex = easeInOutCubic(map(e, 0, 0.75));
    const ezy = smoothstep(map(e, 0.35, 1));
    // Äußere Karten bleiben außerhalb ihres inneren Nachbarn, er überholt sie nie.
    const inner =
      Math.abs(d) > 1.5 ? morphPose(G, d - Math.sign(d), mm).x : h.x;
    const x0 = d > 0 ? Math.max(h.x, inner) : Math.min(h.x, inner);
    const p: Pose = {
      x: lerp(x0, s.x, ex),
      y: lerp(h.y, s.y, ezy),
      z: lerp(h.z, s.z, ezy),
      rz: s.rz * ex,
      ry: lerp(h.ry, s.ry, ex),
      rx: 0,
    };
    // Die hinteren Karten bleiben hinter der Front, bis sie seitlich an ihr vorbei sind.
    if (rank > 0.01 && Math.abs(p.x) < 0.55 * lay.Ws)
      p.z = Math.min(p.z, -0.05 * lay.Ws);
    return p;
  }

  function opacityOf(d: number, mm: number): number {
    const a = Math.abs(d);
    const oh = profile(d >= 0 ? lay.heroProfile : lay.heroNeg, a);
    const os = d >= 0 ? profile(lay.stagePos, a) : profile(lay.stageNeg, a);
    // Beide Enden sind vor ±3.5 bei 0, damit der Sprung über das Ringende unsichtbar bleibt.
    return lerp(oh, os, smootherstep(map(mm, 0.05, 0.9)));
  }

  // ---- Schutz gegen Durchdringung ---------------------------------------------------

  /** Ein Punkt (u, v, w) eines Fensters in Rig-Koordinaten, wie der CSS-Transform ihn setzt. */
  function winPoint(p: Pose, u: number, v: number, w = 0): Vec {
    let q: Vec = [u, v, w];
    if (Math.abs(p.rx) > 0.01) {
      const hh = lay.H / 2;
      q = rotX([q[0], q[1] - hh, q[2]], p.rx);
      q = [q[0], q[1] + hh, q[2]];
    }
    q = rotY(q, p.ry);
    q = rotZ(q, p.rz);
    return [q[0] + p.x, q[1] + p.y, q[2] + p.z];
  }

  /**
   * Wie weit die innere Kante von b (Ecken oben, Mitte, unten) vor der Ebene von a liegt,
   * gezählt nur dort, wo sie über a steht. Liefert die Tiefe und die z-Komponente der
   * Normale von a, um eine Tiefe in eine Verschiebung nach hinten umzurechnen.
   */
  function penetration(b: Pose, u: number, a: Pose): [number, number] {
    const half = lay.Ws / 2;
    const hh = lay.H / 2;
    const c = winPoint(a, 0, 0);
    const ex = winPoint(a, 1, 0);
    const ez = winPoint(a, 0, 0, 1);
    const tx: Vec = [ex[0] - c[0], ex[1] - c[1], ex[2] - c[2]];
    const nz: Vec = [ez[0] - c[0], ez[1] - c[1], ez[2] - c[2]];
    let depth = -Infinity;
    for (const v of [-hh, 0, hh]) {
      const k = winPoint(b, u, v);
      const rel: Vec = [k[0] - c[0], k[1] - c[1], k[2] - c[2]];
      if (Math.abs(dot(rel, tx)) > half + 2) continue;
      depth = Math.max(depth, dot(rel, nz));
    }
    return [depth, Math.max(0.3, nz[2])];
  }

  /**
   * Schutz gegen Durchdringung, in 3D an den Ecken der inneren Kanten gemessen (die Rolle
   * der Fenster schrägt die Kanten an). Für jedes Nachbarpaar weicht das äußere Fenster
   * nach hinten aus, bis seine Vorderseite hinter der Rückwand des inneren liegt. Mitten
   * in einer Drehung wechselt, welches Fenster außen ist; dort geht die Verschiebung
   * stetig von einem auf das andere über, damit nichts springt.
   */
  function guard(poses: Pose[], ds: number[]) {
    const margin = lay.T + 6;
    const half = lay.Ws / 2;
    const idx = ds
      .map((_, i) => i)
      .filter((i) => Math.abs(ds[i]) <= 3.2)
      .sort((a, b) => ds[a] - ds[b]);
    const pairs: [number, number][] = [];
    for (let k = 0; k + 1 < idx.length; k++) {
      const a = idx[k];
      const b = idx[k + 1];
      if (Math.abs(ds[b] - ds[a] - 1) < 0.01) pairs.push([a, b]);
    }
    const inner = (q: [number, number]) =>
      Math.min(Math.abs(ds[q[0]]), Math.abs(ds[q[1]]));
    pairs.sort((p, q) => inner(p) - inner(q));
    for (const [a, b] of pairs) {
      // a liegt links von b; w ist das Gewicht dafür, dass b das äußere Fenster ist.
      const w = smoothstep(map(Math.abs(ds[b]) - Math.abs(ds[a]), -0.3, 0.3));
      const [db, nb] = penetration(poses[b], -half, poses[a]);
      const [da, na] = penetration(poses[a], half, poses[b]);
      const pushB = w * Math.max(0, db + margin);
      const pushA = (1 - w) * Math.max(0, da + margin);
      poses[b].z -= pushB / nb;
      poses[a].z -= pushA / na;
      if (dev && Math.max(pushA, pushB) > margin + 40)
        console.warn(
          `stage guard: push ${Math.max(pushA, pushB).toFixed(1)}px`,
        );
    }
  }

  // ---- Licht ----------------------------------------------------------------------

  /**
   * a ist die Deckkraft der Blätter, b die Helligkeit. Gedimmte Fenster werden dunkel,
   * nicht durchsichtig: sonst schimmern die hinteren Konturen durch den Bildschirm.
   */
  function light(
    win: HTMLElement,
    G: Params,
    p: Pose,
    d: number,
    sx: number,
    mm: number,
    rank: number,
    a0: number,
    b0: number,
    soft: number,
  ) {
    let nv: Vec = [0, 0, 1];
    nv = rotX(nv, p.rx);
    nv = rotY(nv, p.ry);
    nv = rotZ(nv, p.rz);
    nv = rotZ(nv, G.beta);
    nv = rotX(nv, G.tilt);
    const diffuse = clamp(0.35 + 0.65 * dot(nv, LIGHT_FLAT));
    const spec = reduced ? 0.4 : Math.pow(Math.max(0, dot(nv, SPEC_H)), 14);
    const a = Math.abs(d);
    let shade = Math.min(
      0.72,
      0.55 * (1 - diffuse) + 0.15 * Math.max(0, a - 0.5),
    );
    if (a < 0.5) shade = Math.min(shade, 0.05);
    if (lay.stack) {
      // Im Stapel liegen die hinteren Karten im Schatten der Front.
      const stackShade = Math.min(0.72, 0.26 * a);
      shade = lerp(stackShade, shade, smoothstep(map(mm, 0.2, 0.9)));
    }
    const yaw = p.ry;
    // Die Kante leuchtet, sobald das Fenster aus der Blickachse dreht, um Y wie im
    // Einflug um X; frontal bleibt sie ruhig.
    const turned = Math.hypot(
      Math.sin(yaw * RAD),
      Math.sin(p.rx * RAD) * Math.cos(yaw * RAD),
    );
    const lit = clamp(turned * 1.4) * (0.5 + 0.5 * diffuse);
    const gx =
      (clamp(-yaw / 45, -1, 1) * 0.6 + (0.25 * (sx - vw / 2)) / vw) * lay.Ws;
    const glint = 0.18 + 0.77 * spec;
    // Begleitlicht im Morph: läuft von links nach rechts über die Fenster, die Front zuerst.
    let swx = 0;
    let swo = 0;
    if (!reduced) {
      const s = smoothstep(map(mm, 0.45 + 0.05 * rank, 0.85 + 0.05 * rank));
      swx = (2 * s - 1) * lay.Ws;
      swo = 4 * s * (1 - s);
    }
    const halo =
      (1 - clamp(a)) * (0.55 + 0.45 * smootherstep(map(mm, 0.05, 0.9)));
    put(win, "--shade", (1 - (1 - shade) * b0).toFixed(3));
    // Unscharfe Fenster tragen keine harte Lichtkante.
    put(win, "--lit", (lit * a0 * a0 * b0 * (1 - 0.8 * soft)).toFixed(3));
    put(win, "--spec", glint.toFixed(3));
    put(win, "--gx", `${gx.toFixed(1)}px`);
    put(win, "--swx", `${swx.toFixed(1)}px`);
    put(win, "--swo", swo.toFixed(3));
    put(win, "--halo", halo.toFixed(3));
  }

  // ---- Einflug --------------------------------------------------------------------

  // Startzeiten je Hero-Abstand d, von vorn nach hinten; jedes spätere Fenster kommt hinter
  // schon gelandeten an. Was im Fächer unsichtbar steht, fliegt nie.
  const FAN_DELAY: Record<string, number> = {
    "0": 0,
    "1": 300,
    "2": 540,
    "3": 740,
  };
  const STACK_DELAY: Record<string, number> = { "2": 0, "1": 160, "0": 320 };

  function prepareIntro() {
    const G = lay.hero;
    introStarts = [];
    introDelay = [];
    introOrder = [];
    wins.forEach((_, i) => {
      const d = Math.round(wrap(i - pos));
      const P = morphPose(G, d, 0);
      const key = String(d);
      let start: Pose;
      let delay: number;
      if (lay.stack) {
        delay = STACK_DELAY[key] ?? -1;
        start = { ...P, y: P.y + (vh * 0.1) / G.S, rx: 28 };
      } else if (d === 0) {
        delay = 0;
        start =
          lay.cls === "G"
            ? { ...P, x: P.x + (vw * 0.06) / G.S, ry: P.ry - 14 }
            : {
                ...P,
                y: P.y + (vh * 0.3) / G.S,
                z: P.z - 0.5 * lay.Ws,
                rx: 44,
                ry: P.ry - 10,
              };
      } else {
        delay = FAN_DELAY[key] ?? -1;
        start = {
          ...P,
          x: 0.15 * P.x,
          y: P.y + (vh * 0.08) / G.S,
          z: P.z - 0.35 * lay.Ws,
          rx: 14,
          ry: 0.2 * P.ry,
        };
      }
      introStarts.push(start);
      introDelay.push(delay);
      introOrder.push(Math.abs(d));
    });
  }

  /** Mischt Start und Ziel je Kanal mit seiner Feder; liefert die Deckkraft des Einflugs. */
  function introBlend(i: number, P: Pose, now: number): [Pose, number] {
    const delay = introDelay[i];
    const t0 = introT0 + (delay < 0 ? 820 : delay);
    const t = (now - t0) / 1000;
    if (delay < 0) return [P, clamp(t / 0.42)];
    if (t <= 0) return [introStarts[i], 0];
    const uT = springU(t, 6.5, 1);
    const uR = springU(t, 5.2, 0.8);
    const s = introStarts[i];
    return [
      {
        x: lerp(s.x, P.x, uT),
        y: lerp(s.y, P.y, uT),
        z: lerp(s.z, P.z, uT),
        rz: P.rz,
        ry: lerp(s.ry, P.ry, uR),
        rx: lerp(s.rx, P.rx, uR),
      },
      clamp(t / 0.42),
    ];
  }

  // ---- Beschriftung ---------------------------------------------------------------

  interface Layer {
    el: HTMLElement;
    name: HTMLElement;
    head: HTMLElement;
    text: HTMLElement;
    streak: HTMLElement;
    words: HTMLElement[];
    group: number[];
    line: number[];
    anims: Animation[];
    shown: number;
  }

  const layers: Layer[] = layerEls.map((el) => ({
    el,
    name: el.querySelector<HTMLElement>("[data-cap-name]")!,
    head: el.querySelector<HTMLElement>("[data-cap-title]")!,
    text: el.querySelector<HTMLElement>("[data-cap-text]")!,
    streak: el.querySelector<HTMLElement>("[data-streak]")!,
    words: [],
    group: [],
    line: [],
    anims: [],
    shown: -1,
  }));
  let front = 0;
  // Welches Modul die Beschriftung zeigt; sie folgt active, aber nicht in jeder Bewegung.
  let capShown = 0;
  const changes: number[] = [];
  let burstTimer = 0;
  let enterTimer = 0;
  let capScroll = true;
  const easeIn = bezier(0.16, 1, 0.3, 1);

  /** Zerlegt einen Text in Wort-Masken; nur Präsentation, der Text bleibt derselbe. */
  function words(el: HTMLElement, text: string) {
    el.textContent = "";
    const parts = text.split(" ");
    parts.forEach((word, k) => {
      const mask = document.createElement("span");
      mask.className = "w";
      const inner = document.createElement("span");
      inner.className = "wi";
      inner.textContent = word;
      mask.append(inner);
      el.append(mask);
      if (k < parts.length - 1) el.append(" ");
    });
  }

  function fillLayer(l: Layer, idx: number) {
    const win = wins[idx];
    for (const a of l.anims) a.cancel();
    l.anims = [];
    words(l.name, win.dataset.name ?? "");
    words(l.head, win.dataset.title ?? "");
    words(l.text, win.dataset.text ?? "");
    l.words = [...l.el.querySelectorAll<HTMLElement>(".wi")];
    l.group = l.words.map((w) =>
      l.name.contains(w) ? 0 : l.head.contains(w) ? 1 : 2,
    );
    l.shown = idx;
    l.el.style.visibility = "";
    l.el.style.opacity = "";
    // Zeilen braucht nur die Beschriftung am Telefon; das Messen erzwingt ein Layout, und
    // fillLayer läuft mitten in der Drehung.
    if (phoneCaption()) measureLines(l);
  }

  /** Zeilennummern für den Zeilen-Wischer am Telefon, aus der Lage der Masken. */
  function measureLines(l: Layer) {
    let lastTop = -1e9;
    let line = -1;
    l.line = l.words.map((w) => {
      const top = (w.parentElement as HTMLElement).offsetTop;
      if (Math.abs(top - lastTop) > 4) {
        line++;
        lastTop = top;
      }
      return line;
    });
  }

  function hideLayer(l: Layer) {
    l.el.style.visibility = "hidden";
  }

  function settleAnims(l: Layer) {
    for (const a of l.anims) {
      try {
        a.commitStyles();
      } catch {
        /* nicht gerenderte Elemente können nichts übernehmen */
      }
      a.cancel();
    }
    l.anims = [];
  }

  function clearInline(l: Layer) {
    for (const w of l.words) {
      w.style.transform = "";
      w.style.opacity = "";
    }
  }

  const phoneCaption = () => lay.cls === "F";

  function animateIn(l: Layer, r: number, delay: number, fast = false) {
    clearInline(l);
    const phone = phoneCaption();
    let k = 0;
    let lastGroup = 0;
    let extra = 0;
    // Langer Fließtext: der Versatz je Wort schrumpft, damit der Text nach gut 1 s steht.
    const bodyWords = l.group.filter((g) => g === 2).length;
    const bodyStep = Math.min(22, 240 / Math.max(1, bodyWords));
    l.words.forEach((w, i) => {
      const group = l.group[i];
      if (group !== lastGroup) {
        k = 0;
        lastGroup = group;
        extra += group === 2 ? 80 + 22 * 6 : 22 * 4;
      }
      // Im schnellen Drehen steigen alle Wörter zugleich und kurz auf: der Text ist sofort da.
      const at = fast
        ? delay
        : phone
          ? delay + l.line[i] * 60
          : delay + extra + k * (group === 2 ? bodyStep : 22);
      const from = phone
        ? `translate3d(${r * 24}px, 100%, 0)`
        : `translate3d(0, ${r * 110}%, 0) rotate(${r * 4}deg)`;
      l.anims.push(
        w.animate(
          [
            { transform: from, opacity: 0 },
            { transform: "translate3d(0, 0, 0) rotate(0deg)", opacity: 1 },
          ],
          {
            duration: fast ? 260 : phone ? 560 : 640,
            delay: at,
            easing: "cubic-bezier(0.16, 1, 0.3, 1)",
            fill: "backwards",
          },
        ),
      );
      k++;
    });
  }

  function animateOut(l: Layer, r: number) {
    // Von unten nach oben: der Fließtext geht zuerst, damit die neue Überschrift nie auf
    // altem Text landet. Die ganze Schicht ist nach 200 ms ausgeblendet.
    const step = Math.min(14, 120 / Math.max(1, l.words.length));
    const last = l.words.length - 1;
    l.anims.push(
      l.el.animate([{ opacity: 1 }, { opacity: 0 }], {
        duration: 160,
        delay: 40,
        easing: "cubic-bezier(0.55, 0, 0.75, 0.2)",
        fill: "forwards",
      }),
    );
    l.words.forEach((w, j) => {
      const i = last - j;
      l.anims.push(
        w.animate(
          [
            { transform: "translate3d(0, 0, 0)", opacity: 1 },
            { transform: `translate3d(0, ${-r * 105}%, 0)`, opacity: 0.2 },
          ],
          {
            duration: 220,
            delay: i * step,
            easing: "cubic-bezier(0.55, 0, 0.75, 0.2)",
            fill: "forwards",
          },
        ),
      );
    });
    const token = l.shown;
    Promise.all(l.anims.map((x) => x.finished)).then(
      () => {
        if (layers[front] !== l && l.shown === token) hideLayer(l);
      },
      () => undefined,
    );
  }

  // Die Ansage für Screenreader kommt erst, wenn der Ring steht, und nur bei einem anderen
  // Modul als zuletzt angesagt: Beim Ziehen über mehrere Fenster wäre sie sonst ein Strom
  // von Zwischenständen.
  let announced = -1;
  function announce(idx: number) {
    if (idx === announced) return;
    announced = idx;
    const win = wins[idx];
    live.textContent = `${win.dataset.name ?? ""}. ${win.dataset.title ?? ""} ${win.dataset.text ?? ""}`;
  }
  function announceSettled() {
    announce(mod(Math.round(pos)));
  }

  /**
   * Wechsel der Beschriftung; r ist die Drehrichtung. Bei einem einzelnen Schritt (Pfeil,
   * Taste, Selbstlauf) mitten in der Bewegung, beim Ziehen, Wischen und mehreren Schritten
   * erst, wenn der Ring steht (render). Das Zählwerk läuft immer mit.
   */
  function caption(idx: number, r: number) {
    if (!dragging && !springOn) announce(idx);
    capShown = idx;
    if (capScroll) {
      // Im Morph treibt der Scrollweg die Beschriftung; nur den Inhalt tauschen.
      fillLayer(layers[front], idx);
      return;
    }
    const now = performance.now();
    changes.push(now);
    while (changes.length && now - changes[0] > 400) changes.shift();
    const burst = changes.length >= 3;
    const old = layers[front];
    const neu = layers[1 - front];
    if (reduced) {
      fillLayer(old, idx);
      return;
    }
    settleAnims(old);
    settleAnims(neu);
    front = 1 - front;
    fillLayer(neu, idx);
    if (burst) {
      // Schnelles Drehen: Texte tauschen sofort ohne Ausgang, jeder Stand steigt gleich
      // auf; der letzte behält seinen Eingang. Der Lichtstrich läuft erst am Ende.
      hideLayer(old);
      animateIn(neu, r, 0, true);
      clearTimeout(burstTimer);
      burstTimer = window.setTimeout(sweepStreak, 160);
      return;
    }
    animateOut(old, r);
    clearTimeout(enterTimer);
    animateIn(neu, r, 110);
    sweepStreak();
  }

  function sweepStreak() {
    if (reduced) return;
    layers[front].streak.animate(
      [
        { transform: "translate3d(-100%, 0, 0)", opacity: 0 },
        { opacity: 0.9, offset: 0.35 },
        { transform: "translate3d(260%, 0, 0)", opacity: 0 },
      ],
      { duration: 900, easing: "ease-out" },
    );
  }

  /** Erster Auftritt der Beschriftung, getrieben vom Morph statt von der Zeit. */
  function captionScroll(ce: number) {
    const l = layers[front];
    const phone = phoneCaption();
    const span = 640 + 22 * 14 + 80;
    const t = ce * span;
    let k = 0;
    let lastGroup = 0;
    let extra = 0;
    l.words.forEach((w, i) => {
      const group = l.group[i];
      if (group !== lastGroup) {
        k = 0;
        lastGroup = group;
        extra += group === 2 ? 80 + 22 * 2 : 22 * 3;
      }
      const at = phone ? l.line[i] * 60 : extra + k * 22;
      const e = easeIn(clamp((t - at) / 640));
      const r = 1 - e;
      put(
        w,
        "transform",
        r < 0.001
          ? "none"
          : phone
            ? `translate3d(${(r * 24).toFixed(1)}px, ${(r * 100).toFixed(1)}%, 0)`
            : `translate3d(0, ${(r * 110).toFixed(1)}%, 0) rotate(${(r * 4).toFixed(2)}deg)`,
      );
      put(w, "opacity", e.toFixed(3));
      k++;
    });
    const head = clamp(ce * 2.2);
    put(capHead, "opacity", head.toFixed(3));
    put(
      capHead,
      "transform",
      `translate3d(0, ${((1 - easeIn(head)) * 18).toFixed(1)}px, 0)`,
    );
  }

  // Der Index als Zählwerk: zwei Ziffernspalten, die Einer laufen in Drehrichtung weiter.
  let odoQ = 1;
  let odoV = 0;
  let odoTarget = 1;
  let odoShown = 0;
  const odoStrip = odo.querySelector<HTMLElement>("[data-odo-ones]")!;
  // Höhe einer Ziffer in px (1em der Spalte); measure() setzt sie je Bildausschnitt.
  let odoEm = 0;

  function odoTo(idx: number, r: number) {
    const from = ((Math.round(odoTarget) % 10) + 10) % 10;
    const to = idx + 1;
    let dq: number;
    const jump = Math.abs(wrap(idx - odoShown));
    if (jump > 3 || r === 0) {
      dq = ((((to - from) % 10) + 15) % 10) - 5;
    } else if (r > 0) {
      dq = (((to - from) % 10) + 10) % 10;
    } else {
      dq = -((((from - to) % 10) + 10) % 10);
    }
    odoShown = idx;
    odoTarget += dq;
    if (reduced) {
      odoQ = odoTarget;
      odoV = 0;
    }
  }

  function odoStep(h: number): boolean {
    const w = 9;
    const z = 0.75;
    const a = -w * w * (odoQ - odoTarget) - 2 * z * w * odoV;
    odoV += a * h;
    odoQ += odoV * h;
    if (Math.abs(odoQ - odoTarget) < 0.001 && Math.abs(odoV) < 0.01) {
      odoQ = odoTarget;
      odoV = 0;
      return false;
    }
    return true;
  }

  // ---- Karussell ------------------------------------------------------------------

  // Dauer und Kurve eines Wechsels: schnell los, lang und weich aus.
  const TURN_MS = 720;
  const easeTurn = bezier(0.22, 1, 0.36, 1);

  /** Der Ring steht: ansagen (außer nach dem Selbstlauf) und den Selbstlauf weiterlaufen lassen. */
  function settled() {
    if (!lastTurnAuto) announceSettled();
    syncAuto();
  }

  function springStep(h: number) {
    const w = 11;
    const z = 0.86;
    const a = -w * w * (pos - target) - 2 * z * w * vel;
    vel += a * h;
    pos += vel * h;
    if (Math.abs(pos - target) < 0.0005 && Math.abs(vel) < 0.002) {
      // Präzises Einrasten: am Ende steht pos exakt auf dem Ziel.
      pos = target;
      vel = 0;
      springOn = false;
      settled();
    }
  }

  /** Die laufenden Kurven eines Wechsels auswerten; fertige gehen in die Basis über. */
  function tweenStep(now: number) {
    tweens = tweens.filter((tw) => {
      if (now - tw.t0 < TURN_MS) return true;
      tweenBase += tw.delta;
      return false;
    });
    if (!tweens.length) {
      multiTurn = false;
      pos = target;
      tweenBase = target;
      settled();
      return;
    }
    let p = tweenBase;
    for (const tw of tweens) p += tw.delta * easeTurn((now - tw.t0) / TURN_MS);
    pos = p;
  }

  /** Bricht Kurven und Feder ab; der Ring bleibt, wo er gerade ist. */
  function holdRing() {
    tweens = [];
    multiTurn = false;
    tweenBase = pos;
    springOn = false;
    vel = 0;
  }

  const canTurn = () => liveState;
  let lastTurnAuto = false;

  function turn(by: number, auto = false) {
    if (!canTurn()) return;
    clearTimeout(wheelTimer);
    steering = false;
    lastTurnAuto = auto;
    // Nach Ziehen oder Wischen zählt die Position; während eine Fahrt läuft, zählt ein
    // weiterer Klick von ihrem Ziel aus weiter.
    if (!tweens.length) {
      holdRing();
      target = Math.round(pos);
    }
    const goal = clamp(target + by, Math.round(pos) - 3, Math.round(pos) + 3);
    const dest = tweens.reduce((sum, tw) => sum + tw.delta, tweenBase);
    target = goal;
    if (reduced) {
      holdRing();
      pos = target;
      tweenBase = target;
      settled();
    } else if (Math.abs(goal - dest) > 1e-6) {
      tweens.push({ t0: performance.now(), delta: goal - dest });
      if (tweens.length > 1) multiTurn = true;
    }
    // Ein Eingriff von Hand startet die Zeit neu, auch wenn er den Fokus am Ende nicht wechselt.
    if (!auto) restartAuto();
    schedule();
  }

  /** Welches Fenster den Vortritt hat: das Ziel, beim Ziehen das in Zugrichtung kommende. */
  function liftIndex(): number {
    if (!steering) return mod(Math.round(target));
    return mod(steerDir > 0 ? Math.ceil(pos - 1e-3) : Math.floor(pos + 1e-3));
  }

  // ---- Selbstlauf -----------------------------------------------------------------
  // Der aktive Abschnitt füllt sich in AUTO_MS, dann dreht der Ring eins weiter. Er hält, solange
  // der Zeiger über einem Fenster steht, die Tastatur in der Steuerung ist, der Tab verborgen
  // oder die Bühne aus dem Bild ist, und bei reduzierter Bewegung läuft er nie. Die Füllung ist
  // eine Web Animation: pausiert hält sie ihren Stand, und pro Bild rechnet kein Skript.

  const AUTO_MS = 5000;
  const autoOn =
    !reduced && segs.length === n && typeof segs[0]?.animate === "function";
  let autoAnim: Animation | null = null;
  let hoverWins = false;
  let keyFocus = false;

  const inMotion = () =>
    tweens.length > 0 || springOn || dragging || steering;

  function autoAllowed() {
    return (
      liveState &&
      inView &&
      !document.hidden &&
      !hoverWins &&
      !keyFocus &&
      !inMotion()
    );
  }

  // Ohne den Cache von put(): commitStyles() schreibt den Stil an ihm vorbei. Die Werte gehen
  // über eine Transition im CSS, so springt beim Wechsel kein Abschnitt.
  function paintSegs() {
    segs.forEach((seg, i) => {
      seg.style.transform =
        i < active || ((!autoOn || reduced) && i === active)
          ? "scaleX(1)"
          : "scaleX(0)";
    });
  }

  /** Füllstand eines Abschnitts, wie er gerade gemalt ist. */
  function segScale(seg: HTMLElement): number {
    const t = getComputedStyle(seg).transform;
    return !t || t === "none" ? 0 : new DOMMatrixReadOnly(t).a;
  }

  // Ein neu gestarteter Abschnitt läuft erst in dieser Zeit leer, statt auf 0 zu springen.
  const DRAIN_MS = 240;

  /** Neuer Abschnitt oder Eingriff von Hand: die Füllung beginnt von vorn. */
  function restartAuto() {
    // Den gemalten Stand übernehmen, bevor die Animation geht; sonst fiele der alte Abschnitt
    // für ein Bild auf seinen Grundwert zurück und liefe dann erst zum neuen.
    const from = autoAnim ? segScale(segs[active]) : 0;
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
    if (!autoOn || reduced || !liveState) return;
    const drain = from > 0.01 ? DRAIN_MS : 0;
    const total = AUTO_MS + drain;
    const anim = segs[active].animate(
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

  /** Steht der Punkt (Viewport) über einem sichtbaren Fenster? Aus den Posen des letzten Bildes. */
  function overWindow(x: number, y: number): boolean {
    if (!lastG) return false;
    const G = lastG;
    return lastPoses.some((p, i) => {
      if (lastOps[i] < 0.05) return false;
      const q = [-1, 1].flatMap((sx) =>
        [-1, 1].map((sy) =>
          project(
            toWorld(G, winPoint(p, (sx * lay.Ws) / 2, (sy * lay.H) / 2)),
          ),
        ),
      );
      const xs = q.map((v) => v[0]);
      const ys = q.map((v) => v[1]);
      return (
        x >= Math.min(...xs) &&
        x <= Math.max(...xs) &&
        y >= Math.min(...ys) &&
        y <= Math.max(...ys)
      );
    });
  }
  let lastG: Params | null = null;
  let lastPoses: Pose[] = [];
  let lastOps: number[] = [];

  // Das Video läuft nur, wenn sein Fenster nah am Fokus und sichtbar ist, die Bühne im Bild
  // und der Tab vorn; bei reduzierter Bewegung bleibt das Bild stehen (stage/video.ts).
  reducedQuery.addEventListener("change", (event) => {
    reduced = event.matches;
    restartAuto();
    video.setReduced(reduced);
  });

  // ---- Ein Bild -------------------------------------------------------------------

  function render(now: number) {
    frame = 0;
    const dt = lastNow ? Math.min(0.1, (now - lastNow) / 1000) : 1 / 60;
    lastNow = now;
    let busy = false;

    // Folger: der Morph läuft dem Scrollweg als Feder hinterher und springt nie, solange man
    // die Bühne sieht (stage/follow.mjs). Außer Sicht steht er sofort am Ziel, etwa nach einem
    // Sprung über einen Anker.
    if (reduced) setMorph(mRaw >= 0.5 ? 1 : 0);
    else if (!stageOnScreen()) setMorph(mRaw);
    else {
      follow.m = m;
      busy = followStep(follow, mRaw, dt) || busy;
      m = follow.m;
    }

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
    // Der Vortritt wechselt in rund 50 ms: lange bevor sich gehendes und kommendes Fenster
    // auf halbem Weg in gleicher Tiefe träfen, liegt das kommende schon davor.
    const liftAt = liftIndex();
    const kLift = reduced ? 1 : 1 - Math.exp(-dt / 0.05);
    for (let i = 0; i < n; i++) {
      const goal = i === liftAt ? 1 : 0;
      liftW[i] += (goal - liftW[i]) * kLift;
      if (Math.abs(goal - liftW[i]) < 1e-3) liftW[i] = goal;
      else busy = true;
    }
    if (odoQ !== odoTarget) busy = odoStep(Math.min(dt, 1 / 30)) || busy;

    const intro = introT0 >= 0 && !introDone;
    const tIntro = intro ? now - introT0 : 1e9;
    if (intro && now - lastFrameStamp < 200) {
      introFrames++;
      introFrameSum += now - lastFrameStamp;
      if (introFrames === 30 && introFrameSum / 30 > 20 && !lite) {
        lite = true;
        scene.dataset.lite = "";
      }
    }
    lastFrameStamp = now;

    const G = mixG(m);
    if (intro) G.S *= 0.94 + 0.06 * easeOutCubic(clamp(tIntro / 2200));
    put(
      rig,
      "transform",
      `translate3d(${G.cx.toFixed(1)}px, ${G.cy.toFixed(1)}px, 0) rotateX(${G.tilt.toFixed(3)}deg) rotateZ(${G.beta.toFixed(3)}deg) scale(${G.S.toFixed(4)})`,
    );

    // Posen aller Fenster, dann Schutz, dann schreiben.
    const ds = wins.map((_, i) => wrap(i - pos));
    const poses: Pose[] = [];
    const ops: number[] = [];
    let allLanded = true;
    ds.forEach((d, i) => {
      let p = morphPose(G, d, m, liftW[i]);
      let o = opacityOf(d, m);
      if (introT0 >= 0 && !introDone) {
        const [q, io] = introBlend(i, p, now);
        p = q;
        o *= io;
        const delay = introDelay[i];
        if (now - introT0 - Math.max(0, delay) < 2000) allLanded = false;
      } else if (introT0 < 0 && !reduced) {
        o = 0;
        allLanded = false;
      }
      poses.push(p);
      ops.push(o);
    });
    guard(poses, ds);
    lastG = G;
    lastPoses = poses;
    lastOps = ops;

    // Fenster jenseits von d = +1 blenden aus, sobald sie in das Band der Navigation steigen:
    // dort schimmerte der Bildschirm durch die halbtransparente Leiste.
    const band = 0.05 * vh + 24;
    ds.forEach((d, i) => {
      const wgt = smoothstep(map(Math.abs(d), 1, 2));
      if (wgt <= 0 || ops[i] <= 0) return;
      const p = poses[i];
      const top = Math.min(
        ...[-1, 1].map(
          (sx) =>
            project(toWorld(G, winPoint(p, (sx * lay.Ws) / 2, -lay.H / 2)))[1],
        ),
      );
      const fade = clamp((top - (navBottom + 24 - band)) / band);
      ops[i] *= lerp(1, fade, wgt);
    });

    // Reihenfolge links nach rechts für das Begleitlicht.
    const screenX = poses.map((p) => project(toWorld(G, [p.x, p.y, p.z]))[0]);
    const byX = ds.map((_, i) => i).sort((a, b) => screenX[a] - screenX[b]);
    const visibleRank = new Array<number>(n).fill(0);
    let r = 0;
    for (const i of byX) if (ops[i] > 0.01) visibleRank[i] = r++;
    const frontRank = visibleRank[ds.findIndex((d) => Math.abs(d) < 0.5)] ?? 0;

    // Das Video läuft nur nah am Fokus; weiter hinten deckt die weiche Ebene es ganz zu.
    if (videoWin >= 0)
      video.setShown(ops[videoWin] > 0.05 && Math.abs(ds[videoWin]) < 1.25);

    // Tiefenschärfe erst auf der Bühne: im Fächer bleibt alles scharf.
    const focusOn = smoothstep(map(m, 0.55, 0.95));
    const halfH = lay.H / 2;
    wins.forEach((win, i) => {
      const p = poses[i];
      const o = ops[i];
      const tilt =
        Math.abs(p.rx) > 0.01
          ? ` translate3d(0, ${halfH.toFixed(1)}px, 0) rotateX(${p.rx.toFixed(2)}deg) translate3d(0, ${(-halfH).toFixed(1)}px, 0)`
          : "";
      put(
        win,
        "transform",
        `translate3d(-50%, -50%, 0) translate3d(${p.x.toFixed(1)}px, ${p.y.toFixed(1)}px, ${p.z.toFixed(1)}px) rotateZ(${p.rz.toFixed(3)}deg) rotateY(${p.ry.toFixed(3)}deg)${tilt}`,
      );
      // Der Körper erscheint früh und ganz, die Konturen folgen steiler: kein Drahtgitter.
      const a = clamp(o / 0.35);
      const b = a > 0 ? o / a : 0;
      put(win, "--o", a.toFixed(3));
      const off = Math.abs(ds[i]);
      // Die Unschärfe steigt sanft mit der Tiefe: bei |d| = 1 knapp zur Hälfte, ab 2 ganz.
      const soft = focusOn * smoothstep(map(off, 0.1, 2.2));
      put(win, "--oe", (a * a * a * b * (1 - 0.6 * soft)).toFixed(3));
      put(win, "visibility", o <= 0.002 ? "hidden" : "");
      // Die Schärfe wandert mit dem Fokus: die weiche Ebene ist am Fokus exakt 0. Ihr Filter
      // ist fest, pro Bild ändert sich nur ihre opacity.
      if (o > 0.002) {
        put(dofs[i], "opacity", soft.toFixed(3));
        put(feathers[i], "opacity", (0.5 * soft * a).toFixed(3));
      }
      if (o > 0.002)
        light(
          win,
          G,
          p,
          ds[i],
          screenX[i],
          m,
          Math.abs(visibleRank[i] - frontRank),
          a,
          b,
          soft,
        );
    });
    busy ||= intro && !allLanded;
    if (intro && allLanded) introDone = true;

    // Der Ring: Boden der Bühne, mit den Fenstern gerollt und gekippt. Die Größe folgt
    // dem Radius über scale. Die Teilstriche stehen still: fast von der Kante gesehen
    // ließ ihr Drehen das feine Muster als wandernde Strichellinie flimmern.
    const R = G.r * lay.Ws;
    const ringScale =
      (R + 0.06 * lay.Ws) / (lay.stage.r * lay.Ws + 0.06 * lay.Ws);
    // Die Fenster rollen nur zum Teil mit dem Bogen: ihre untere Ecke senkt sich unter die
    // Bodenlinie. Der Ring liegt um diese Senkung tiefer, sonst schnitte seine Vorderkante
    // als Strich durch den Bildschirm.
    const rho0 = lerp(lay.hero.rho, lay.stage.rho, smoothstep(clamp(m / 0.88)));
    const rz0 = Math.abs((1 - rho0) * G.beta) * RAD;
    const dip =
      (lay.Ws / 2) * Math.sin(rz0) + (lay.H / 2) * (1 - Math.cos(rz0));
    put(
      ring,
      "transform",
      `translate3d(-50%, -50%, 0) translate3d(0, ${(lay.H / 2 + lay.T + 2 + dip).toFixed(1)}px, ${(-R).toFixed(1)}px) rotateX(90deg) scale(${ringScale.toFixed(4)})`,
    );
    const ringIn = reduced ? 1 : introT0 < 0 ? 0 : clamp((tIntro - 500) / 1200);
    // Telefon quer: auf der Bühne läge der flach gesehene Ring quer über der Beschriftung.
    // Telefon hoch: dort lief die Vorderkante des fast flach gesehenen Rings als gestrichelte
    // Linie quer durch Zählwerk und Beschriftung.
    const ringStage = lay.cls === "G" || lay.stack ? 0 : 0.8;
    // Im Fächer sieht man den Ring in A, F und G fast von der Kante: dort flimmerte er als
    // Strichellinie unter den Scheiben.
    const ringHero = lay.cls === "A" || lay.stack || lay.cls === "G" ? 0 : 0.32;
    put(
      ring,
      "opacity",
      (
        ringIn * lerp(ringHero, ringStage, smoothstep(map(m, 0.3, 0.9)))
      ).toFixed(3),
    );
    const nearest = Math.min(...ds.map((d) => Math.abs(d)));
    put(
      pool,
      "opacity",
      (smoothstep(map(m, 0.5, 1)) * (1 - clamp(nearest))).toFixed(3),
    );
    put(
      spot,
      "transform",
      `translate3d(${(0.6 * G.cx).toFixed(1)}px, ${(0.4 * G.cy).toFixed(1)}px, 0)`,
    );
    put(spot, "opacity", lerp(1, 0.8, m).toFixed(3));

    // Der Nebel gehört zum Anfang: er kommt mit dem Einflug und ist bei m = 0,3 ganz weg.
    // Danach liegt er nicht mehr im Layout und belegt keine Ebenen.
    const fogOff = m >= FOG_END;
    if (fogOff !== fogGone) {
      fogGone = fogOff;
      fog.hidden = fogOff;
    }
    if (!fogOff) {
      const fogIn = reduced ? 1 : introT0 < 0 ? 0 : clamp(tIntro / 1800);
      put(
        fog,
        "opacity",
        (fogIn * (1 - smoothstep(map(m, 0, FOG_END)))).toFixed(3),
      );
      for (const layer of fogLayers) {
        const depth = fogDepth[layer.dataset.fogLayer!];
        // Die Strahlen hängen wie der Lichtkegel an der Bühne, der Nebel nur an ihrer Fahrt.
        const x = depth === fogDepth.rays ? G.cx : G.cx - lay.hero.cx;
        put(
          layer,
          "transform",
          `translate3d(${(depth * x).toFixed(1)}px, ${(depth * (G.cy - lay.hero.cy)).toFixed(1)}px, 0)`,
        );
      }
    }

    // Überschrift und Lede gehen schneller als das Rig nach oben: Kran-Parallaxe.
    put(heroText, "opacity", (1 - easeOutQuad(map(m, 0, 0.38))).toFixed(3));
    put(
      heroText,
      "transform",
      `translate3d(0, ${(-0.12 * vh * easeInCubic(map(m, 0, 0.6))).toFixed(1)}px, 0)`,
    );
    const ctaOut = map(m, 0, 0.22);
    put(actions, "opacity", (1 - ctaOut).toFixed(3));
    put(
      actions,
      "transform",
      `translate3d(0, ${(16 * ctaOut).toFixed(1)}px, 0)`,
    );
    put(actions, "visibility", m > 0.25 ? "hidden" : "");

    // Die Beschriftung: erster Auftritt über den Scrollweg, danach eigene Übergänge.
    const ce = map(m, 0.62, 0.95);
    const scrollMode = m < 0.95;
    if (scrollMode !== capScroll) {
      capScroll = scrollMode;
      if (scrollMode) {
        settleAnims(layers[0]);
        settleAnims(layers[1]);
        clearTimeout(burstTimer);
        hideLayer(layers[1 - front]);
        // Neue Wort-Spans, damit keine übernommenen Stile der Übergänge stehen bleiben.
        fillLayer(layers[front], active);
      } else {
        clearInline(layers[front]);
        put(capHead, "opacity", "1");
        put(capHead, "transform", "none");
      }
    }
    if (capScroll) captionScroll(reduced ? (m >= 0.5 ? 1 : 0) : ce);
    put(cap, "visibility", m > 0.6 ? "visible" : "hidden");
    put(controls, "opacity", map(m, 0.8, LIVE_AT).toFixed(3));
    const isLive = m >= LIVE_AT;
    if (isLive !== liveState) {
      liveState = isLive;
      deck.inert = !isLive;
      scene.toggleAttribute("data-live", isLive);
      // Auf der Bühne beginnt der Selbstlauf mit einem frischen Abschnitt, im Fächer hält er.
      if (isLive) restartAuto();
      else syncAuto();
    }

    // Aktives Modul: Wechsel mitten im Ziehen oder Drehen, synchron zur Bewegung.
    const nowActive = mod(Math.round(pos));
    if (nowActive !== active) {
      const dir = Math.sign(wrap(nowActive - active)) || 1;
      active = nowActive;
      scene.dataset.active = String(active);
      wins.forEach((win, i) =>
        win.setAttribute("aria-hidden", i === active ? "false" : "true"),
      );
      odoTo(active, dir);
    }
    // Die Beschriftung wechselt bei einem einzelnen Schritt mitten in der Bewegung, sonst erst,
    // wenn die Hand losgelassen hat und der Ring steht. Früher tauschte jeder überstrichene
    // Index Text, Wort-Masken und Abschnitte mitten im Ziehen: beim schnellen Wischen blitzten
    // Texte auf und verschwanden ohne Übergang, und die Füllung der Abschnitte sprang.
    // Mehrere Schritte bleiben mehrere, bis die letzte Kurve steht (multiTurn), auch wenn die
    // erste schon fertig ist. Das gilt auch im Morph ab LIVE_AT, wo capScroll noch gilt.
    const handMotion = dragging || steering || springOn || multiTurn;
    if (capShown !== active && !handMotion) {
      caption(active, Math.sign(wrap(active - capShown)) || 1);
      restartAuto();
    }

    const digit = ((odoQ % 10) + 10) % 10;
    // In Ruhe auf ganze Pixel: die Spalte ist eine eigene Ebene, ein Bruchteil ließe die
    // Konturziffern unscharf stehen.
    put(
      odoStrip,
      "transform",
      odoQ === odoTarget
        ? `translate3d(0, ${-Math.round(digit * odoEm)}px, 0)`
        : `translate3d(0, ${(-digit).toFixed(4)}em, 0)`,
    );

    if (busy) schedule();
  }
  let lastFrameStamp = 0;

  function schedule() {
    if (!frame) frame = requestAnimationFrame(render);
  }

  // Der Zeiger schiebt die Ebenen um wenige Pixel; den weichen Lauf macht eine Transition auf
  // translate. Ganze Pixel und höchstens ein Schreiben je Bild halten das billig.
  let fogPointer = 0;
  let fogX = 0;
  let fogY = 0;
  if (!reduced && window.matchMedia("(hover: hover) and (pointer: fine)").matches) {
    window.addEventListener(
      "pointermove",
      (event) => {
        fogX = event.clientX / vw - 0.5;
        fogY = event.clientY / vh - 0.5;
        if (fogPointer || lite || fogGone) return;
        fogPointer = requestAnimationFrame(() => {
          fogPointer = 0;
          for (const layer of fogLayers) {
            const depth = fogDepth[layer.dataset.fogLayer!];
            put(
              layer,
              "translate",
              `${Math.round(-28 * depth * fogX)}px ${Math.round(-16 * depth * fogY)}px`,
            );
          }
        });
      },
      { passive: true },
    );
  }

  // ---- Scrollen -------------------------------------------------------------------

  /** Setzt den Morph ohne Lauf, die Feder steht. */
  function setMorph(value: number) {
    m = value;
    follow.m = value;
    follow.v = 0;
  }

  /** Liegt der angeheftete Abschnitt im Bild? Aus gecachten Maßen, ohne Layout zu lesen. */
  function stageOnScreen(): boolean {
    const p = window.scrollY - sceneTop;
    return p > -vh && p < sceneH;
  }

  function read() {
    const p = window.scrollY - sceneTop;
    mRaw = clamp((p - lay.hold * vh) / (lay.morph * vh));
    if (reduced) mRaw = clamp(p / (0.6 * vh));
  }

  window.addEventListener(
    "scroll",
    () => {
      read();
      // Liegt die fertige Bühne weit unterhalb des Bildes, und dreht sich nichts mehr, gibt
      // es nichts zu malen: kein Bild je Scroll-Ereignis für eine unsichtbare Bühne. Die Höhe
      // kommt aus measure(); ein Lesen von offsetHeight erzwänge hier je Ereignis ein Layout.
      const past = window.scrollY - sceneTop - sceneH > vh;
      if (past && m >= 1 && !springOn && !tweens.length && !dragging) {
        // Ein Sprung am Morph vorbei: ohne Bild käme der Nebel sonst nie aus dem Layout.
        if (!fogGone) {
          fogGone = true;
          fog.hidden = true;
        }
        return;
      }
      schedule();
    },
    { passive: true },
  );

  // Neu vermessen höchstens einmal je Bild und nur, wenn sich Breite oder die Höhe des
  // angehefteten Abschnitts wirklich geändert haben. Die Adressleiste eines Telefons ändert nur
  // innerHeight; früher baute das die Bühne 150 ms später mitten im Morph neu auf.
  let resizeFrame = 0;
  window.addEventListener("resize", () => {
    if (resizeFrame) return;
    resizeFrame = requestAnimationFrame(() => {
      resizeFrame = 0;
      if (window.innerWidth !== vw || stableHeight() !== vh) {
        measure();
        if (phoneCaption())
          for (const l of layers) if (l.words.length) measureLines(l);
        // Der Zug läuft weiter: von hier aus mit dem neuen Maßstab, ohne Sprung.
        if (dragMoved && dragging) {
          pos0 = pos;
          ax = lastX;
          ay = lastY;
        }
      }
      read();
      schedule();
    });
  });

  // ---- Eingaben -------------------------------------------------------------------

  scene.querySelector("[data-prev]")!.addEventListener("click", () => turn(-1));
  scene.querySelector("[data-next]")!.addEventListener("click", () => turn(1));

  // Pfeiltasten nur, wenn die Bühne mindestens die Hälfte des Viewports deckt.
  let inView = false;
  new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        inView = e.intersectionRatio >= 0.5;
        video.setSeen(e.isIntersecting);
        // Die Lichtebenen sind eigene Ebenen, solange die Bühne im Bild ist (Home.astro).
        scene.toggleAttribute("data-awake", e.isIntersecting);
      }
      syncAuto();
    },
    { threshold: [0, 0.5, 1] },
  ).observe(sticky);
  document.addEventListener("visibilitychange", () => {
    syncAuto();
    video.sync();
  });
  // Zeiger über einem Fenster hält den Selbstlauf; nur die Maus, ein Finger schwebt nicht.
  const setHover = (on: boolean) => {
    if (on === hoverWins) return;
    hoverWins = on;
    syncAuto();
  };
  hit.addEventListener("pointermove", (event) => {
    if (event.pointerType === "mouse")
      setHover(overWindow(event.clientX, event.clientY));
  });
  hit.addEventListener("pointerleave", () => setHover(false));
  // Tastaturfokus in der Steuerung hält ihn auch; ein Mausklick auf einen Pfeil nicht.
  deck.addEventListener("focusin", (event) => {
    const el = event.target as HTMLElement;
    let visible = false;
    try {
      visible = el.matches(":focus-visible");
    } catch {
      /* ältere Browser kennen :focus-visible nicht */
    }
    keyFocus = visible;
    syncAuto();
  });
  deck.addEventListener("focusout", (event) => {
    if (deck.contains(event.relatedTarget as Node | null)) return;
    keyFocus = false;
    syncAuto();
  });
  document.addEventListener("keydown", (event) => {
    if (
      !inView ||
      !canTurn() ||
      event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      event.shiftKey
    )
      return;
    const el = event.target as HTMLElement | null;
    if (el?.closest("input, textarea, select, [contenteditable]")) return;
    if (event.key === "ArrowLeft") turn(-1);
    else if (event.key === "ArrowRight") turn(1);
    else return;
    event.preventDefault();
  });

  // Ziehen mit Maus und Finger: der Ring folgt dem Zeiger 1:1 entlang der Bogentangente,
  // beim Loslassen fließt der Schwung in die Feder, die genau auf ein Fenster einrastet.
  let dragging = false;
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
    if (!canTurn() || dragging) return;
    if (event.pointerType === "mouse" && event.button !== 0) return;
    if ((event.target as Element).closest("button, a")) return;
    if (event.pointerType === "mouse") {
      // Sonst startet der Browser einen nativen Bild- oder Text-Drag und bricht ab.
      event.preventDefault();
      hit.setPointerCapture(event.pointerId);
    }
    dragging = true;
    dragMoved = false;
    dragId = event.pointerId;
    ax = event.clientX;
    ay = event.clientY;
  });

  hit.addEventListener("pointermove", (event) => {
    if (!dragging || event.pointerId !== dragId) return;
    lastX = event.clientX;
    lastY = event.clientY;
    const dx = event.clientX - ax;
    const dy = event.clientY - ay;
    if (!dragMoved) {
      if (Math.hypot(dx, dy) < 6) return;
      // Ein Finger, der eher senkrecht läuft, scrollt die Seite (touch-action: pan-y). Früher
      // drehte auch er den Ring an, bis der Browser mit pointercancel übernahm; dann federte
      // der Ring zurück, und der Selbstlauf begann von vorn: jedes Scrollen über die Bühne
      // ließ sie zucken.
      if (event.pointerType !== "mouse" && Math.abs(dy) >= Math.abs(dx)) {
        dragging = false;
        return;
      }
      // Ab der Schwelle gilt der ganze Weg seit dem Aufsetzen, damit der Ring 1:1 folgt.
      dragMoved = true;
      holdRing();
      pos0 = pos;
      samples = [];
      steering = true;
      scene.toggleAttribute("data-dragging", true);
      syncAuto();
    }
    const b = mixG(m).beta * RAD;
    const next = pos0 - (dx * Math.cos(b) + dy * Math.sin(b)) / pxPerStep;
    if (pos === pos0) steerLatch.reset(next - pos0);
    steerDir = steerLatch.push(next - pos);
    pos = next;
    const t = performance.now();
    samples.push({ t, p: pos });
    while (samples.length > 2 && t - samples[0].t > 80) samples.shift();
    schedule();
  });

  function release(cancelled: boolean) {
    if (!dragging) return;
    dragging = false;
    scene.toggleAttribute("data-dragging", false);
    if (!dragMoved) {
      syncAuto();
      return;
    }
    steering = false;
    lastTurnAuto = false;
    swallowClick = true;
    window.setTimeout(() => (swallowClick = false), 0);
    let v = 0;
    if (!cancelled && samples.length >= 2) {
      const a = samples[0];
      const z = samples[samples.length - 1];
      if (performance.now() - z.t < 80 && z.t > a.t)
        v = ((z.p - a.p) / (z.t - a.t)) * 1000;
    }
    // Der Ring steht, wo der Zeiger ihn hingezogen hat; begrenzt wird nur der Schwung
    // danach, höchstens drei Fenster weiter (HUM-334, Review).
    target = Math.round(pos + clamp(v * 0.32, -3, 3));
    if (reduced) {
      pos = target;
      vel = 0;
      announceSettled();
    } else {
      vel = v;
      springOn = true;
    }
    restartAuto();
    schedule();
  }
  hit.addEventListener(
    "pointerup",
    (event) => event.pointerId === dragId && release(false),
  );
  hit.addEventListener(
    "pointercancel",
    (event) => event.pointerId === dragId && release(true),
  );
  hit.addEventListener(
    "lostpointercapture",
    (event) => event.pointerId === dragId && release(false),
  );
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

  // Waagrechtes Trackpad-Scrollen dreht live; senkrecht bleibt immer Sache der Seite. Die Achse
  // gilt für die ganze Geste samt Nachlauf, entschieden an ihren ersten Pixeln
  // (stage/gesture.mjs); ein einzelnes rauschendes Ereignis dreht nichts mehr.
  const wheelAxis = createWheelAxis();
  sticky.addEventListener(
    "wheel",
    (event) => {
      const [px, py] = wheelPixels(event, vw);
      const axis = wheelAxis(px, py, performance.now());
      if (axis === "y" || !canTurn()) return;
      if (axis === null) {
        // Noch offen: nicht drehen, aber eine waagrechte Geste nicht als Blättern im Verlauf
        // an den Browser geben.
        if (Math.abs(px) > Math.abs(py)) event.preventDefault();
        return;
      }
      event.preventDefault();
      const delta = px / pxPerStep;
      if (!steering) steerLatch.reset(delta);
      holdRing();
      steering = true;
      lastTurnAuto = false;
      pos += delta;
      steerDir = steerLatch.push(delta);
      tweenBase = pos;
      syncAuto();
      clearTimeout(wheelTimer);
      wheelTimer = window.setTimeout(() => {
        target = Math.round(pos);
        steering = false;
        if (reduced) {
          pos = target;
          announceSettled();
        } else springOn = true;
        restartAuto();
        schedule();
      }, WHEEL_IDLE_MS);
      schedule();
    },
    { passive: false },
  );

  // Für Prüfskripte: der Zustand der Bühne, nur lesend.
  Object.defineProperty(scene, "__stage", {
    value: () => ({
      m,
      mRaw,
      pos,
      target,
      vel,
      cls: lay.cls,
      pxPerStep,
      active,
      springOn,
      tweening: tweens.length > 0,
      auto: autoAnim
        ? { state: autoAnim.playState, t: autoAnim.currentTime }
        : null,
      liftW: [...liftW],
      lift: liftIndex(),
      introDone,
      introAge: introT0 > 0 ? performance.now() - introT0 : -1,
      stage: { ...lay.stage, Ws: lay.Ws },
    }),
  });

  // ---- Start ----------------------------------------------------------------------

  measure();
  read();
  setMorph(reduced ? (mRaw >= 0.5 ? 1 : 0) : mRaw);
  deck.inert = true;
  if (!reduced) fog.dataset.drift = "";
  // Jede Ebene kommt, sobald ihr Bild dekodiert ist; der Einflug wartet nicht darauf.
  // Far und near haben eine Fassung je Thema: Die andere lädt erst, wenn ein Themenwechsel sie
  // ins Layout bringt, dann blendet sie genauso auf (HUM-338).
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
  const fogPause = () => fog.toggleAttribute("data-paused", document.hidden);
  document.addEventListener("visibilitychange", fogPause);
  fogPause();
  wins.forEach((_, i) => liftW.push(i === 0 ? 1 : 0));
  paintSegs();
  fillLayer(layers[0], 0);
  hideLayer(layers[1]);
  wins.forEach((win, i) =>
    win.setAttribute("aria-hidden", i === 0 ? "false" : "true"),
  );
  scene.dataset.active = "0";

  if (reduced || mRaw > 0.3) {
    // Ohne Einflug: die Pose steht sofort.
    introT0 = 0;
    introDone = true;
    schedule();
  } else {
    // Start erst, wenn Bild und Schriften da sind, höchstens nach 700 ms, dann 80 ms Luft.
    const gate = Promise.all([
      frontImg?.decode().catch(() => undefined),
      document.fonts?.ready.catch(() => undefined),
    ]);
    const cap700 = new Promise((resolve) => setTimeout(resolve, 700));
    Promise.race([gate, cap700]).then(() => {
      window.setTimeout(() => {
        measure();
        read();
        prepareIntro();
        introT0 = performance.now();
        lastNow = 0;
        schedule();
      }, 80);
    });
    schedule();
  }

  // Schriften und Bilder können die Höhe oben noch verschieben.
  window.addEventListener("load", () => {
    measure();
    read();
    schedule();
  });
}

// Start erst hier unten: die Konstanten oben müssen vorher stehen.
const scene = document.querySelector<HTMLElement>("[data-scene]");
if (scene) init(scene);
