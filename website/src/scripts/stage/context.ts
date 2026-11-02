// Gemeinsamer Zustand der Bühne: die Elemente, die Maße des Bildausschnitts und der Zustand
// von Morph und Ring. Jedes Modul unter stage/ bekommt dieses eine Objekt; wer etwas ändert,
// ändert es hier, damit kein Modul eine eigene Kopie veralten lässt.

import type { Params } from "./geometry.mjs";
import type { Profile } from "./math.mjs";

export type Cls = "A" | "B" | "C" | "D" | "E" | "F" | "G";

export interface Layout {
  cls: Cls;
  Ws: number;
  H: number;
  T: number;
  P: number;
  hero: Params;
  stage: Params;
  morph: number;
  heroProfile: Profile;
  heroNeg: Profile;
  stagePos: Profile;
  stageNeg: Profile;
  stack: boolean;
  /** Text links, Stapel rechts (Breitbild und Telefon quer). */
  side: boolean;
  /** Beschriftung unter der Bühne (Hochformat). */
  below: boolean;
  /** Steuerung mittig unter der Front. */
  ctlUnder: boolean;
  /** Kleinster Abstand der Fenster zur Beschriftung in px (Prüfskripte). */
  capGap: number;
  /** Lage der Steuerung unter der Front (Mitte, Oberkante, Breite), sonst null. */
  ctl: { x: number; y: number; w: number } | null;
  /** Oberkante der Beschriftung unter der Bühne, sonst null. */
  capTop: number | null;
}

export interface Refs {
  scene: HTMLElement;
  sticky: HTMLElement;
  world: HTMLElement;
  rig: HTMLElement;
  ring: HTMLElement;
  pool: HTMLElement;
  /** Die sieben Module, in der Reihenfolge des Rings. */
  wins: HTMLElement[];
  /** Das Fenster mit dem Ablauf, nur im Stapel der Startseite (HUM-413). */
  lead: HTMLElement | null;
  heroText: HTMLElement;
  actions: HTMLElement;
  hint: HTMLElement | null;
  spot: HTMLElement;
  fog: HTMLElement;
  hit: HTMLElement;
  deck: HTMLElement;
  cap: HTMLElement;
  capHead: HTMLElement;
  layersBox: HTMLElement;
  layerEls: HTMLElement[];
  controls: HTMLElement;
  segs: HTMLElement[];
  odo: HTMLElement;
  odoStrip: HTMLElement;
  live: HTMLElement;
  nav: HTMLElement | null;
  frontImg: HTMLImageElement | null;
  dofs: HTMLElement[];
  feathers: HTMLElement[];
  videoEl: HTMLVideoElement | null;
}

export function queryRefs(scene: HTMLElement): Refs {
  const one = <T extends HTMLElement = HTMLElement>(q: string) =>
    scene.querySelector<T>(q)!;
  const wins = [...scene.querySelectorAll<HTMLElement>("[data-win]")];
  const lead = scene.querySelector<HTMLElement>("[data-lead]");
  const odo = one("[data-odo]");
  return {
    scene,
    sticky: one("[data-sticky]"),
    world: one("[data-world]"),
    rig: one("[data-rig]"),
    ring: one("[data-ring]"),
    pool: one("[data-pool]"),
    wins,
    lead,
    heroText: one("[data-hero]"),
    actions: one("[data-hero-actions]"),
    hint: scene.querySelector<HTMLElement>("[data-scroll-hint]"),
    spot: one("[data-spot]"),
    fog: one("[data-fog]"),
    hit: one("[data-hit]"),
    deck: one("[data-deck]"),
    cap: one("[data-cap]"),
    capHead: one("[data-cap-head]"),
    layersBox: one(".cap__layers"),
    layerEls: [...scene.querySelectorAll<HTMLElement>("[data-layer]")],
    controls: one("[data-controls]"),
    segs: [...scene.querySelectorAll<HTMLElement>("[data-seg]")],
    odo,
    odoStrip: odo.querySelector<HTMLElement>("[data-odo-ones]")!,
    live: one("[data-live-text]"),
    nav: document.querySelector<HTMLElement>("[data-nav] .nav__bar, [data-nav]"),
    frontImg: (lead ?? wins[0])?.querySelector("img") ?? null,
    dofs: wins.map((w) => w.querySelector<HTMLElement>("[data-dof]")!),
    feathers: wins.map((w) => w.querySelector<HTMLElement>("[data-feather]")!),
    videoEl: scene.querySelector<HTMLVideoElement>("[data-video]"),
  };
}

export interface State {
  n: number;
  /** Modul im Fokus, wenn die Bühne ankommt; oben steht es direkt hinter dem Ablauf. */
  start: number;
  vw: number;
  vh: number;
  lay: Layout;
  sceneTop: number;
  sceneH: number;
  pxPerStep: number;
  navBottom: number;
  /** Morph roh aus dem Scrollweg und geglättet. */
  mRaw: number;
  m: number;
  /** Lage des Rings auf der Bühne (Index des Fensters im Fokus, gleitend). */
  pos: number;
  /** Schritte, die der Ring beim Zurückscrollen dreht, mit Vorzeichen (choreography.mjs). */
  steps: number;
  reduced: boolean;
  live: boolean;
  inView: boolean;
  lite: boolean;
  active: number;
  dragging: boolean;
  steering: boolean;
  introT0: number;
  introDone: boolean;
}

/** Schreibt Stil nur, wenn sich der Wert geändert hat. */
export type Put = (el: HTMLElement, prop: string, value: string) => void;

export function createPut(): Put {
  const cache = new WeakMap<HTMLElement, Record<string, string>>();
  return (el, prop, value) => {
    let c = cache.get(el);
    if (!c) cache.set(el, (c = {}));
    if (c[prop] === value) return;
    c[prop] = value;
    if (prop.startsWith("--")) el.style.setProperty(prop, value);
    else (el.style as unknown as Record<string, string>)[prop] = value;
  };
}

export interface Ctx {
  refs: Refs;
  s: State;
  put: Put;
  /** Ein Bild anfordern; mehrere Anforderungen bis zum nächsten Bild fallen zusammen. */
  schedule: () => void;
}
