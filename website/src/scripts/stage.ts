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
// nur über Pfeile, Ziehen, Wischen, waagrechtes Scrollen und die Pfeiltasten, ohne Ende
// in beide Richtungen.
//
// Die Geometrie hängt an einem Parametersatz G (Größe, Lage, Kippung, Rolle des Bogens,
// Winkelabstand, Radius). Der Morph mischt Parameter, nicht Posen: so bleibt die
// Reihenfolge der Fenster in jedem Zwischenzustand erhalten. Pro Bild ändern sich nur
// transform, opacity und Custom Properties, die Deckkraft und Verschiebung von
// Lichtebenen steuern. Ein einziger rAF-Loop läuft nur, solange sich etwas bewegt.

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

const ASPECT = 2560 / 1380;
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
const STAGE_NEG: Profile = [
  [0, 1],
  [1, 0.42],
  [1.6, 0],
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
  const ticks = scene.querySelector<HTMLElement>("[data-ticks]")!;
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
  const fill = scene.querySelector<HTMLElement>("[data-fill]")!;
  const odo = scene.querySelector<HTMLElement>("[data-odo]")!;
  const live = scene.querySelector<HTMLElement>("[data-live-text]")!;
  const layerEls = [...scene.querySelectorAll<HTMLElement>("[data-layer]")];
  const nav = document.querySelector<HTMLElement>(
    "[data-nav] .nav__bar, [data-nav]",
  );
  const frontImg = wins[0]?.querySelector("img");
  // Die Unschärfe: je Fenster die weiche Ebene, ihre Skalierung und die Lichtscheiben.
  const dofs = wins.map((w) => w.querySelector<HTMLElement>("[data-dof]")!);
  const dofInner = wins.map((w) =>
    w.querySelector<HTMLElement>("[data-dof-in]")!,
  );
  const bokehs = wins.map((w) => w.querySelector<HTMLElement>("[data-bokeh]")!);
  const feathers = wins.map((w) =>
    w.querySelector<HTMLElement>("[data-feather]")!,
  );
  const n = wins.length;
  const reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const dev = import.meta.env.DEV;

  // Abstand auf dem Ring, gefaltet in [-n/2, n/2): jedes Fenster hat links und rechts Nachbarn.
  const wrap = (d: number) => ((((d + n / 2) % n) + n) % n) - n / 2;
  const mod = (v: number) => ((v % n) + n) % n;

  let vw = 0;
  let vh = 0;
  let lay!: Layout;
  let sceneTop = 0;
  let pxPerStep = 400;
  let navBottom = 0;

  // Zustand: Morph (roh aus dem Scrollweg und geglättet), Ringposition, Einflug.
  let mRaw = 0;
  let m = 0;
  let pos = 0;
  let vel = 0;
  let target = 0;
  let springOn = false;
  let wheelTimer = 0;
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
        Ws = Math.min(vw * 0.47, vh * 1.12);
        hero = params({
          tilt: 4,
          phis: 15,
          r: 1.3,
          kappa: 0.27,
          delta: 0.22,
          yaw: -32,
        });
        stage = params({
          cy: -vh * 0.06,
          tilt: 3,
          beta: -14,
          rho: 0.55,
          phis: 30,
          r: 1.2,
          kappa: 1,
          delta: 0,
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
        Ws = Math.min(vw * 0.52, vh * 0.98);
        hero = params(FAN);
        stage = params({
          cy: -vh * 0.1,
          tilt: 2,
          beta: -15,
          rho: 0.55,
          phis: 33,
          r: 1.08,
          kappa: 1,
          delta: 0,
        });
        stagePos = PHONE_POS;
        break;
      case "C":
        Wh = Math.min(vw * 0.46, fanRoom(1));
        Ws = Math.min(vw * 0.53, vh * 0.9);
        hero = params(FAN);
        stage = params({
          cy: -vh * 0.12,
          tilt: 2,
          beta: -15,
          rho: 0.55,
          phis: 33,
          r: 1.08,
          kappa: 1,
          delta: 0,
        });
        stagePos = PHONE_POS;
        break;
      case "D":
        Wh = Math.min(vw * 0.52, fanRoom(1));
        Ws = vw * 0.6;
        hero = params({ ...FAN, phis: 14 });
        stage = params({
          cy: -vh * 0.15,
          tilt: 3,
          beta: -13,
          rho: 0.55,
          phis: 33,
          r: 1.05,
          kappa: 1,
          delta: 0,
        });
        // Bei 30° Winkelabstand stünde d = 3 schon hinter dem Scheitel des Bogens.
        stagePos = PHONE_POS;
        break;
      case "E":
        Wh = vw * 0.76;
        Ws = vw * 0.87;
        hero = params({ ...FAN, phis: 10, delta: 0.26 });
        stage = params({
          cy: -vh * 0.16,
          tilt: 3,
          beta: -14,
          rho: 0.55,
          phis: 32,
          r: 1.15,
          kappa: 1,
          delta: 0,
        });
        stagePos = PHONE_POS;
        break;
      case "F":
        Wh = Math.min(vw * 0.86, vh * 0.3 * ASPECT);
        Ws = vw * 0.93;
        hero = params({ tilt: 4 });
        stage = params({
          cy: -vh * 0.16,
          tilt: 3,
          beta: -5,
          rho: 0.6,
          phis: 17,
          r: 2.4,
          kappa: 0.55,
          delta: 0,
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
        Ws = Math.min(vw * 0.5, vh * 0.68 * ASPECT);
        hero = params({ ...FAN, phis: 12, delta: 0.2, yaw: -30 });
        stage = params({
          cy: vh * 0.06,
          tilt: 2,
          beta: -8,
          rho: 0.55,
          phis: 28,
          r: 1.2,
          kappa: 1,
          delta: 0,
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
      if (top >= need && !capHit) break;
      const high = top < need;
      if (high && stage.beta < betaMin - 0.01)
        stage.beta = Math.min(betaMin, stage.beta + 1);
      else if (high && !capHit) stage.cy += Math.min(need - top, 0.02 * vh) + 1;
      else if (capHit && top > need + 2) {
        // Die Beschriftung stößt an: die Bühne steigt in den freien Raum unter der Navigation.
        stage.cy -= Math.min(top - need, 0.02 * vh);
      } else Ws *= 0.985;
    }
    hero.S = Wh / Ws;
    const H = Ws / ASPECT;
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
      z: -R * (1 - Math.cos(phi)) - G.delta * Math.abs(d) * Ws,
      rz: -(1 - G.rho) * G.beta,
      ry: -G.kappa * d * G.phis + G.yaw,
      rx: 0,
    };
    return rectWith(G, p, Ws, Ws / ASPECT, perspectiveOf(Ws));
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

  function measure() {
    vw = window.innerWidth;
    vh = window.innerHeight;
    const cls = classify(vw, vh);
    scene.dataset.cls = cls;
    reserveCaption();
    lay = layout(cls);
    scene.style.setProperty("--w", `${lay.Ws.toFixed(1)}px`);
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
    // Pixel pro Fenster entlang des Bogens: projizierter Abstand von Front und d = +1.
    const a = project(toWorld(lay.stage, [0, 0, 0]));
    const b = project(toWorld(lay.stage, slotVec(lay.stage, 1)));
    pxPerStep = clamp(Math.hypot(b[0] - a[0], b[1] - a[1]), 140, 700);
    if (lite) scene.dataset.lite = "";
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

  function slotVec(G: Params, d: number): Vec {
    const phi = d * G.phis * RAD;
    const R = G.r * lay.Ws;
    return [
      R * Math.sin(phi),
      0,
      -R * (1 - Math.cos(phi)) - G.delta * Math.abs(d) * lay.Ws,
    ];
  }

  function slotPose(G: Params, d: number, rho: number): Pose {
    const [x, y, z] = slotVec(G, d);
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
    };
  }

  /** Pose von Fenster i im Morph; ρ ist je Fenster gestaffelt. */
  function morphPose(G: Params, d: number, mm: number): Pose {
    const mi = clamp((mm - 0.04 * Math.abs(d)) / 0.88);
    const rho = lerp(lay.hero.rho, lay.stage.rho, smoothstep(mi));
    if (!lay.stack) return slotPose(G, d, rho);
    // Telefon hoch: der Stapel fächert sich zum Bogen auf, Kanal für Kanal.
    const h = stackPose(d);
    const s = slotPose(lay.stage, d, rho);
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
    measureLines(l);
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

  /** Wechsel des aktiven Moduls, mitten in der Bewegung; r ist die Drehrichtung. */
  function caption(idx: number, r: number) {
    if (!dragging && !springOn) announce(idx);
    odoTo(idx, r);
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
      turning = false;
      announceSettled();
    }
  }

  const canTurn = () => liveState;
  let turning = false;

  function turn(by: number) {
    if (!canTurn()) return;
    // Nach Ziehen oder Wischen zählt die Position, nicht ein altes Ziel; nur während eine
    // Pfeil-Drehung noch läuft, zählt ein weiterer Klick vom Ziel aus weiter.
    clearTimeout(wheelTimer);
    const base = springOn && turning ? Math.round(target) : Math.round(pos);
    turning = true;
    target = clamp(base + by, Math.round(pos) - 3, Math.round(pos) + 3);
    if (reduced) {
      pos = target;
      vel = 0;
      turning = false;
      announceSettled();
    } else springOn = true;
    schedule();
  }

  // ---- Ein Bild -------------------------------------------------------------------

  function render(now: number) {
    frame = 0;
    const dt = lastNow ? Math.min(0.1, (now - lastNow) / 1000) : 1 / 60;
    lastNow = now;
    let busy = false;

    // Folger: der Morph läuft dem Scrollweg weich hinterher.
    if (reduced) m = mRaw >= 0.5 ? 1 : 0;
    else if (Math.abs(mRaw - m) > 0.5) m = mRaw;
    else {
      m += (mRaw - m) * (1 - Math.exp(-dt / 0.09));
      if (Math.abs(mRaw - m) < 1e-4) m = mRaw;
      else busy = true;
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
      let p = morphPose(G, d, m);
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
      const soft = focusOn * smoothstep(map(off, 0.08, 1.2));
      put(win, "--oe", (a * a * a * b * (1 - 0.6 * soft)).toFixed(3));
      put(win, "visibility", o <= 0.002 ? "hidden" : "");
      // Die Schärfe wandert mit dem Fokus: die weiche Ebene ist am Fokus exakt 0 und ab
      // |d| = 1.2 ganz da. Ihr Filter ist fest, pro Bild ändern sich nur opacity und scale.
      if (o > 0.002) {
        put(dofs[i], "opacity", soft.toFixed(3));
        put(feathers[i], "opacity", (soft * a).toFixed(3));
        put(
          dofInner[i],
          "transform",
          `scale(${(1 + 0.04 * clamp(off) + 0.03 * clamp((off - 1) / 2)).toFixed(3)})`,
        );
        put(
          bokehs[i],
          "opacity",
          (focusOn * smoothstep(map(off, 0.6, 1.8)) * o).toFixed(3),
        );
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
    // dem Radius über scale, die Teilstriche drehen mit der Ringposition.
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
    const ringStage = lay.cls === "G" ? 0 : 0.8;
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
    put(ticks, "transform", `rotate(${(pos * G.phis).toFixed(3)}deg)`);
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
      caption(active, dir);
    }

    // Fortschritt: Füllung pro Bild; beim Sprung von 7 auf 1 atmet sie kurz.
    const q = mod(pos + 0.5) - 0.5;
    const scale = clamp((q + 1) / n, 1 / n, 1);
    const toJump = Math.abs(q - (n - 0.5));
    const toJump2 = Math.abs(q + 0.5);
    const breath = 1 - 0.65 * (1 - clamp(Math.min(toJump, toJump2) / 0.3));
    put(fill, "transform", `scaleX(${scale.toFixed(4)})`);
    put(fill, "opacity", breath.toFixed(3));
    const digit = ((odoQ % 10) + 10) % 10;
    put(odoStrip, "transform", `translate3d(0, ${(-digit).toFixed(4)}em, 0)`);

    if (busy) {
      // Die Lichtebenen sind nur in Bewegung eigene Ebenen; kurz nachlaufen, sonst pendelt
      // das Attribut zwischen zwei Scroll-Ereignissen.
      if (!moving) {
        moving = true;
        scene.toggleAttribute("data-moving", true);
      }
      clearTimeout(movingTimer);
      movingTimer = window.setTimeout(() => {
        moving = false;
        scene.toggleAttribute("data-moving", false);
      }, 400);
      schedule();
    }
  }
  let lastFrameStamp = 0;
  let moving = false;
  let movingTimer = 0;

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

  function read() {
    const p = window.scrollY - sceneTop;
    mRaw = clamp((p - lay.hold * vh) / (lay.morph * vh));
    if (reduced) mRaw = clamp(p / (0.6 * vh));
  }

  window.addEventListener(
    "scroll",
    () => {
      read();
      const leaving = mRaw <= 0 || mRaw >= 1;
      if (leaving && Math.abs(mRaw - m) > 0.2) m = mRaw;
      // Liegt die fertige Bühne weit unterhalb des Bildes, und dreht sich nichts mehr, gibt
      // es nichts zu malen: kein Bild je Scroll-Ereignis für eine unsichtbare Bühne.
      const past = window.scrollY - sceneTop - scene.offsetHeight > vh;
      if (past && m >= 1 && !springOn && !dragging) {
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

  let resizeTimer = 0;
  window.addEventListener("resize", () => {
    clearTimeout(resizeTimer);
    resizeTimer = window.setTimeout(() => {
      measure();
      read();
      for (const l of layers) if (l.words.length) measureLines(l);
      schedule();
    }, 150);
  });

  // ---- Eingaben -------------------------------------------------------------------

  scene.querySelector("[data-prev]")!.addEventListener("click", () => turn(-1));
  scene.querySelector("[data-next]")!.addEventListener("click", () => turn(1));

  // Pfeiltasten nur, wenn die Bühne mindestens die Hälfte des Viewports deckt.
  let inView = false;
  new IntersectionObserver(
    (entries) => {
      for (const e of entries) inView = e.intersectionRatio >= 0.5;
    },
    { threshold: [0, 0.5, 1] },
  ).observe(sticky);
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
    const dx = event.clientX - ax;
    const dy = event.clientY - ay;
    if (!dragMoved) {
      if (Math.hypot(dx, dy) < 6) return;
      // Ab der Schwelle gilt der ganze Weg seit dem Aufsetzen, damit der Ring 1:1 folgt.
      dragMoved = true;
      springOn = false;
      vel = 0;
      pos0 = pos;
      samples = [];
      scene.toggleAttribute("data-dragging", true);
    }
    const b = mixG(m).beta * RAD;
    pos = pos0 - (dx * Math.cos(b) + dy * Math.sin(b)) / pxPerStep;
    const t = performance.now();
    samples.push({ t, p: pos });
    while (samples.length > 2 && t - samples[0].t > 80) samples.shift();
    schedule();
  });

  function release(cancelled: boolean) {
    if (!dragging) return;
    dragging = false;
    scene.toggleAttribute("data-dragging", false);
    if (!dragMoved) return;
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

  // Waagrechtes Trackpad-Scrollen dreht live; senkrecht bleibt immer Sache der Seite.
  sticky.addEventListener(
    "wheel",
    (event) => {
      if (!canTurn() || Math.abs(event.deltaX) <= 1.2 * Math.abs(event.deltaY))
        return;
      event.preventDefault();
      springOn = false;
      vel = 0;
      const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? vw : 1;
      pos += (event.deltaX * unit) / pxPerStep;
      clearTimeout(wheelTimer);
      wheelTimer = window.setTimeout(() => {
        target = Math.round(pos);
        turning = false;
        if (reduced) {
          pos = target;
          announceSettled();
        } else springOn = true;
        schedule();
      }, 140);
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
      introDone,
      introAge: introT0 > 0 ? performance.now() - introT0 : -1,
      stage: { ...lay.stage, Ws: lay.Ws },
    }),
  });

  // ---- Start ----------------------------------------------------------------------

  measure();
  read();
  m = reduced ? (mRaw >= 0.5 ? 1 : 0) : mRaw;
  deck.inert = true;
  if (!reduced) fog.dataset.drift = "";
  // Jede Ebene kommt, sobald ihr Bild dekodiert ist; der Einflug wartet nicht darauf.
  for (const layer of fogLayers) {
    layer
      .querySelector("img")!
      .decode()
      .catch(() => undefined)
      .then(() => {
        layer.dataset.ready = "";
      });
  }
  // Ausgelaufen: Pose festschreiben, Animation und Ebenen-Hinweis entfernen.
  fog.addEventListener("animationend", (event) => {
    const img = event.target as HTMLElement;
    for (const animation of img.getAnimations()) {
      animation.commitStyles();
      animation.cancel();
    }
    img.closest<HTMLElement>("[data-fog-layer]")!.dataset.rest = "";
  });
  const fogPause = () => fog.toggleAttribute("data-paused", document.hidden);
  document.addEventListener("visibilitychange", fogPause);
  fogPause();
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
