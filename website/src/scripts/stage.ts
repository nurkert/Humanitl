// Die Bühne der Startseite (HUM-334, HUM-413). Ein angehefteter Abschnitt: Oben steht ein
// Stapel aus Fenstern der App, vorn ein aufgenommener Ablauf. Mit dem ersten Pixel Scrollweg
// verlässt das vordere Fenster das Bild nach links, die übrigen wandern auf einen Bogen, der
// nach rechts oben steigt, und werden zum Karussell; daneben oder darunter steht die große
// Beschriftung. Danach scrollt die Seite normal weiter, kein Skript bewegt sie. Das Karussell
// dreht über Pfeile, Ziehen, Wischen, waagrechtes Scrollen und die Pfeiltasten, ohne Ende in
// beide Richtungen, und von selbst alle fünf Sekunden, solange niemand mit ihm arbeitet.
//
// Diese Datei setzt nur zusammen. Die Teile stehen unter stage/:
//   math.mjs, geometry.mjs   Kurven, Drehungen, Plätze auf dem Bogen, Projektion
//   choreography.mjs         Morph, Ausgang des vorderen Fensters, Zurückspulen, Deckkraft
//   layout-solve.mjs         Größe und Lage der Bühne gegen Beschriftung und Steuerung
//   layout.ts                Klassen je Bildausschnitt, Fächer, Messen
//   guard.mjs                Schutz gegen Durchdringung im Fächer
//   frame.ts                 ein Bild: Posen, Licht, Ring, Text der Startseite
//   carousel.ts, input.ts    Ring, Selbstlauf, Eingaben
//   captions.ts              Beschriftung, Zählwerk, Ansage
//   intro.ts, fog.ts         Einflug und Nebel beim Laden
//   hint.ts                  Knopf, der zum Karussell scrollt
//   follow.mjs, gesture.mjs, video.ts   aus HUM-412 unverändert

import { createCaptions } from "./stage/captions";
import { createCarousel } from "./stage/carousel";
import { modRing } from "./stage/choreography.mjs";
import { type Ctx, type State, createPut, queryRefs } from "./stage/context";
import { createFog } from "./stage/fog";
import { createFrame } from "./stage/frame";
import { bindHint } from "./stage/hint";
import { bindInput } from "./stage/input";
import { createIntro } from "./stage/intro";
import { measure, stableHeight } from "./stage/layout";
import { frontVideo } from "./stage/video";

function init(scene: HTMLElement) {
  const refs = queryRefs(scene);
  const n = refs.wins.length;
  // Kann sich zur Laufzeit ändern; dann halten Selbstlauf und Video sofort an (unten).
  const reducedQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
  const reduced = reducedQuery.matches;
  // Auf der Bühne steht Modul 1 im Fokus; oben im Stapel steht es direkt hinter dem eigenen
  // Fenster mit dem Ablauf (choreography.mjs).
  const start = 0;
  const s = {
    n,
    start,
    pos: start,
    steps: 0,
    reduced,
    live: false,
    inView: false,
    lite: (navigator.hardwareConcurrency ?? 8) <= 4,
    active: modRing(start, n),
    dragging: false,
    steering: false,
    introT0: -1,
    introDone: reduced,
    mRaw: 0,
    m: 0,
    vw: 0,
    vh: 0,
    sceneTop: 0,
    sceneH: 0,
    pxPerStep: 400,
    navBottom: 0,
  } as State;
  let frameId = 0;
  const ctx: Ctx = {
    refs,
    s,
    put: createPut(),
    schedule: () => {
      if (!frameId) frameId = requestAnimationFrame(tick);
    },
  };

  const video = frontVideo(refs.videoEl, reduced);
  const captions = createCaptions(ctx);
  const ring = createCarousel(ctx, (auto) => {
    if (!auto) captions.announceAt(s.pos);
  });
  const intro = createIntro(ctx);
  const fog = createFog(ctx);
  const frame = createFrame(ctx, { ring, captions, intro, fog, video });
  const input = bindInput(ctx, ring, { overWindow: frame.overWindow, beta: frame.beta });
  bindHint(ctx, frame.stageY);

  function tick(now: number) {
    frameId = 0;
    if (frame.render(now)) ctx.schedule();
  }

  reducedQuery.addEventListener("change", (event) => {
    s.reduced = event.matches;
    ring.restartAuto();
    video.setReduced(s.reduced);
  });

  window.addEventListener(
    "scroll",
    () => {
      frame.read();
      // Liegt die fertige Bühne weit unterhalb des Bildes, und dreht sich nichts mehr, gibt es
      // nichts zu malen. Die Höhe kommt aus measure(); hier wird kein Layout gelesen.
      const past = window.scrollY - s.sceneTop - s.sceneH > s.vh;
      if (past && s.m >= 1 && !ring.inMotion()) {
        fog.hide();
        return;
      }
      ctx.schedule();
    },
    { passive: true },
  );

  // Neu vermessen nur, wenn sich Breite oder die Höhe des angehefteten Abschnitts wirklich
  // geändert haben (die Adressleiste ändert nur innerHeight). Während einer fortlaufenden
  // Größenänderung höchstens alle 100 ms, das letzte Maß gilt immer (der Solver kostet einige Millisekunden, layout.ts merkt sich seine letzte Rechnung).
  let resizePending = false;
  let lastMeasure = 0;
  const remeasure = () => {
    resizePending = false;
    lastMeasure = performance.now();
    if (window.innerWidth !== s.vw || stableHeight(refs.sticky) !== s.vh) {
      measure(ctx, captions.reserve);
      captions.remeasure();
      input.rebase();
    }
    frame.read();
    ctx.schedule();
  };
  window.addEventListener("resize", () => {
    if (resizePending) return;
    resizePending = true;
    const wait = Math.max(0, 100 - (performance.now() - lastMeasure));
    window.setTimeout(() => requestAnimationFrame(remeasure), wait);
  });

  // Pfeiltasten und Selbstlauf nur, wenn die Bühne mindestens die Hälfte des Viewports deckt.
  new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        s.inView = e.intersectionRatio >= 0.5;
        video.setSeen(e.isIntersecting);
        // Die Lichtebenen sind eigene Ebenen, solange die Bühne im Bild ist (Home.astro).
        scene.toggleAttribute("data-awake", e.isIntersecting);
      }
      ring.syncAuto();
    },
    { threshold: [0, 0.5, 1] },
  ).observe(refs.sticky);
  document.addEventListener("visibilitychange", () => {
    ring.syncAuto();
    video.sync();
  });

  // Für Prüfskripte: der Zustand der Bühne, nur lesend.
  Object.defineProperty(scene, "__stage", {
    value: () => ({
      m: s.m,
      mRaw: s.mRaw,
      pos: s.pos,
      steps: s.steps,
      target: ring.target,
      vel: ring.vel,
      cls: s.lay.cls,
      pxPerStep: s.pxPerStep,
      active: s.active,
      springOn: ring.springOn,
      tweening: ring.tweening,
      auto: ring.auto,
      introDone: s.introDone,
      introAge: s.introT0 > 0 ? performance.now() - s.introT0 : -1,
      stage: { ...s.lay.stage, Ws: s.lay.Ws },
      capGap: s.lay.capGap,
      range: { top: s.sceneTop, morphPx: frame.stageY() - s.sceneTop, vh: s.vh },
    }),
  });

  // ---- Start ----------------------------------------------------------------------
  measure(ctx, captions.reserve);
  frame.read();
  frame.setMorph(reduced ? (s.mRaw >= 0.5 ? 1 : 0) : s.mRaw);
  refs.deck.inert = true;
  fog.start();
  ring.paintSegs();
  captions.init(s.active);
  refs.wins.forEach((win, i) =>
    win.setAttribute("aria-hidden", i === s.active ? "false" : "true"),
  );
  scene.dataset.active = String(s.active);

  if (reduced || s.mRaw > 0.3) {
    // Ohne Einflug: die Pose steht sofort.
    s.introT0 = 0;
    s.introDone = true;
    ctx.schedule();
  } else {
    // Start erst, wenn Bild und Schriften da sind, höchstens nach 700 ms, dann 80 ms Luft.
    const gate = Promise.all([
      refs.frontImg?.decode().catch(() => undefined),
      document.fonts?.ready.catch(() => undefined),
    ]);
    const cap700 = new Promise((resolve) => setTimeout(resolve, 700));
    Promise.race([gate, cap700]).then(() => {
      window.setTimeout(() => {
        measure(ctx, captions.reserve);
        frame.read();
        intro.prepare(s.start - 1);
        s.introT0 = performance.now();
        ctx.schedule();
      }, 80);
    });
    ctx.schedule();
  }

  // Schriften und Bilder können die Höhe oben noch verschieben.
  window.addEventListener("load", () => {
    measure(ctx, captions.reserve);
    frame.read();
    ctx.schedule();
  });
}

const scene = document.querySelector<HTMLElement>("[data-scene]");
if (scene) init(scene);
