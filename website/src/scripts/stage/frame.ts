// Ein Bild der Bühne: Morph, Ring, Posen, Licht und alles, was davon abhängt. Ein einziger
// rAF-Loop läuft nur, solange sich etwas bewegt; pro Bild ändern sich nur transform, opacity
// und Custom Properties fertiger Ebenen, und kein Layout wird gelesen.

import type { Captions } from "./captions";
import type { Carousel } from "./carousel";
import { assemble, fadeUnderNav, ranks } from "./assemble.mjs";
import {
  CAP_IN,
  CAP_OUT,
  LIVE_AT,
  REWIND,
  SHIFT,
  mixG,
  modRing,
  rewindSteps,
  wrapRing,
} from "./choreography.mjs";
import type { Ctx } from "./context";
import { followStep, type Follow } from "./follow.mjs";
import type { createFog } from "./fog";
import { type Params, type Pose, project, toWorld, winPoint } from "./geometry.mjs";
import { guard } from "./guard.mjs";
import type { Intro } from "./intro";
import { paintLight } from "./light";
import {
  RAD,
  clamp,
  easeInCubic,
  easeOutCubic,
  easeOutQuad,
  lerp,
  map,
  smoothstep,
} from "./math.mjs";
import type { FrontVideo } from "./video";


interface Parts {
  ring: Carousel;
  captions: Captions;
  intro: Intro;
  fog: ReturnType<typeof createFog>;
  video: FrontVideo;
}

export function createFrame(ctx: Ctx, parts: Parts) {
  const { refs, s, put } = ctx;
  const { wins, scene } = refs;
  const { ring, captions, intro, fog, video } = parts;
  const n = s.n;
  const follow: Follow = { m: s.m, v: 0 };
  // Das Fenster mit dem Ablauf malt mit den Modulen, als Eintrag n hinter ihnen; es hat keine
  // weiche Ebene, weil es nie unscharf wird.
  const lead = refs.lead;
  const els = lead ? [...wins, lead] : wins;
  const dofs: (HTMLElement | null)[] = lead ? [...refs.dofs, null] : refs.dofs;
  const feathers: (HTMLElement | null)[] = lead ? [...refs.feathers, null] : refs.feathers;
  let lastNow = 0;
  let lastStamp = 0;
  let introFrames = 0;
  let introFrameSum = 0;
  let lastG: Params | null = null;
  let lastPoses: Pose[] = [];
  let lastOps: number[] = [];

  const view = () => ({ vw: s.vw, vh: s.vh, Ws: s.lay.Ws, H: s.lay.H, P: s.lay.P });

  /** Setzt den Morph ohne Lauf, die Feder steht. */
  function setMorph(value: number) {
    s.m = value;
    follow.m = value;
    follow.v = 0;
  }

  /** Liegt der angeheftete Abschnitt im Bild? Aus gecachten Maßen, ohne Layout zu lesen. */
  function onScreen(): boolean {
    const p = window.scrollY - s.sceneTop;
    return p > -s.vh && p < s.sceneH;
  }

  /** Morph roh aus dem Scrollweg. */
  function read() {
    const p = window.scrollY - s.sceneTop;
    s.mRaw = s.reduced ? clamp(p / (0.6 * s.vh)) : clamp(p / (s.lay.morph * s.vh));
  }

  /** Scrollweg, an dem die Bühne fertig steht (für den Hinweis-Knopf und Prüfskripte). */
  const stageY = () => s.sceneTop + (s.reduced ? 0.6 : s.lay.morph) * s.vh;

  /** Oben im Stapel: der Ring steht wieder am Anfang, unsichtbar, weil die Lage gleich ist. */
  function resetAtTop() {
    const start = s.start;
    if (s.steps === 0 && s.pos === start) return;
    ring.jump(start);
    s.steps = 0;
    setActive(modRing(start, n), 0);
    captions.jump(s.active);
    ring.paintSegs();
  }

  function setActive(idx: number, dir: number) {
    s.active = idx;
    scene.dataset.active = String(idx);
    wins.forEach((win, i) => win.setAttribute("aria-hidden", i === idx ? "false" : "true"));
    if (dir !== 0) captions.odoTo(idx, dir);
  }

  function render(now: number) {
    const dt = lastNow ? Math.min(0.1, (now - lastNow) / 1000) : 1 / 60;
    lastNow = now;
    let busy = false;
    const { lay } = s;

    // Folger: der Morph läuft dem Scrollweg als Feder hinterher und springt nie, solange man
    // die Bühne sieht (follow.mjs). Außer Sicht steht er sofort am Ziel.
    if (s.reduced) setMorph(s.mRaw >= 0.5 ? 1 : 0);
    else if (!onScreen()) setMorph(s.mRaw);
    else {
      follow.m = s.m;
      busy = followStep(follow, s.mRaw, dt) || busy;
      s.m = follow.m;
    }
    const m = s.m;

    busy = ring.step(now, dt) || busy;
    busy = captions.odoStep(Math.min(dt, 1 / 30)) || busy;

    const intro = s.introT0 >= 0 && !s.introDone;
    const tIntro = intro ? now - s.introT0 : 1e9;
    if (intro && now - lastStamp < 200) {
      introFrames++;
      introFrameSum += now - lastStamp;
      if (introFrames === 30 && introFrameSum / 30 > 20 && !s.lite) {
        s.lite = true;
        scene.dataset.lite = "";
      }
    }
    lastStamp = now;

    // Bühne bedienbar ab LIVE_AT. Beim Verlassen merkt sich der Ring, wie weit er zurückspult.
    const isLive = m >= LIVE_AT;
    if (isLive !== s.live) {
      s.live = isLive;
      refs.deck.inert = !isLive;
      scene.toggleAttribute("data-live", isLive);
      // Beim Verlassen der Bühne hält jede Bewegung dort an, wo sie gerade zu sehen ist; von
      // dieser Lage aus dreht der Ring auf dem kürzeren Weg zum Anfang (rewindSteps). Kommt die
      // Bühne zurück, ohne dass oben zurückgesetzt wurde, rastet er auf das nächste Fenster ein.
      if (!isLive) {
        ring.stop();
        s.steps = rewindSteps(s.pos, s.start, n);
      } else if (s.pos !== Math.round(s.pos)) ring.snap(0);
      if (isLive) ring.restartAuto();
      else ring.syncAuto();
    }
    if (!isLive && m <= SHIFT[0]) resetAtTop();

    const G = mixG(lay.hero, lay.stage, m);
    if (intro) G.S *= 0.94 + 0.06 * easeOutCubic(clamp(tIntro / 2200));
    put(
      refs.rig,
      "transform",
      `translate3d(${G.cx.toFixed(1)}px, ${G.cy.toFixed(1)}px, 0) rotateX(${G.tilt.toFixed(3)}deg) rotateZ(${G.beta.toFixed(3)}deg) scale(${G.S.toFixed(4)})`,
    );

    // Posen und Deckkraft aus der Choreografie (assemble.mjs), dann Einflug und Schutz.
    const v = view();
    const { ds, poses, ops, exit, exiting } = assemble(lay, G, v, {
      n,
      pos: s.pos,
      target: ring.target,
      steps: s.steps,
      m,
      lead: !!lead,
      reduced: s.reduced,
    });
    let allLanded = true;
    ds.forEach((_, i) => {
      if (intro) {
        const [q, io] = parts.intro.blend(i, poses[i], now);
        poses[i] = q;
        ops[i] *= io;
        if (!parts.intro.landed(i, now)) allLanded = false;
      } else if (s.introT0 < 0 && !s.reduced) {
        ops[i] = 0;
        allLanded = false;
      }
    });
    // Schutz gegen Durchdringung nur im Fächer; auf der Bühne trägt die Tiefe (guard.mjs).
    guard(poses.slice(0, n), ds.slice(0, n), {
      Ws: lay.Ws,
      H: lay.H,
      T: lay.T,
      weight: 1 - smoothstep(map(m, 0.55, 0.9)),
    });
    lastG = G;
    lastPoses = poses;
    lastOps = ops;
    fadeUnderNav(ds, poses, ops, G, v, s.navBottom);
    const { screenX, rank } = ranks(ds, poses, ops, G, v);

    // Das Video spielt nur im Stapel, solange sein Fenster vorn steht; im Karussell zeigen alle
    // Fenster ihre Aufnahme (HUM-413).
    if (lead) video.setShown(exiting && exit < 0.9 && ops[n] > 0.05);

    // Tiefenschärfe erst auf der Bühne: im Fächer bleibt alles scharf.
    const focusOn = smoothstep(map(m, 0.55, 0.95));
    const halfH = lay.H / 2;
    els.forEach((win, i) => {
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
      // Die Unschärfe steigt sanft mit der Tiefe: bei |d| = 1 knapp zur Hälfte, ab 2 ganz.
      const soft = focusOn * smoothstep(map(Math.abs(ds[i]), 0.1, 2.2));
      put(win, "--oe", (a * a * a * b * (1 - 0.6 * soft)).toFixed(3));
      put(win, "visibility", o <= 0.002 ? "hidden" : "");
      if (o <= 0.002) return;
      // Die Schärfe wandert mit dem Fokus: die weiche Ebene ist am Fokus exakt 0.
      const dof = dofs[i];
      const feather = feathers[i];
      if (dof) put(dof, "opacity", soft.toFixed(3));
      if (feather) put(feather, "opacity", (0.5 * soft * a).toFixed(3));
      paintLight(ctx, win, {
        G,
        p,
        d: ds[i],
        sx: screenX[i],
        m,
        rank: rank[i],
        a0: a,
        b0: b,
        soft,
      });
    });
    busy ||= intro && !allLanded;
    if (intro && allLanded) s.introDone = true;

    paintRing(G, m, tIntro, ds.slice(0, n));
    fog.frame(G, m, tIntro);
    paintHero(m);

    // Die Beschriftung: erscheint als Ganzes, sobald die Bühne fast steht.
    // Unter REWIND[1] beginnt der Ring zurückzudrehen: dann ist die Beschriftung sofort weg,
    // auch wenn ihr Ausgang bei einer schnellen Rückkehr noch liefe.
    if (m >= CAP_IN) captions.reveal(true);
    else if (m < REWIND[1]) captions.reveal(false, true);
    else if (m < CAP_OUT) captions.reveal(false);
    put(refs.controls, "opacity", map(m, 0.8, LIVE_AT).toFixed(3));

    // Aktives Modul: Wechsel mitten im Ziehen oder Drehen, synchron zur Bewegung.
    const nowActive = modRing(Math.round(s.pos), n);
    if (nowActive !== s.active) setActive(nowActive, Math.sign(wrapRing(nowActive - s.active, n)) || 1);
    // Die Beschriftung wechselt bei einem einzelnen Schritt mitten in der Bewegung, sonst erst,
    // wenn die Hand losgelassen hat und der Ring steht (HUM-412).
    const handMotion = s.dragging || s.steering || ring.springOn || ring.multiTurn;
    if (captions.shown !== s.active && !handMotion) {
      captions.change(
        s.active,
        Math.sign(wrapRing(s.active - captions.shown, n)) || 1,
        !s.dragging && !ring.springOn && !ring.tweening,
      );
      ring.restartAuto();
    }
    captions.paintOdo();
    return busy;
  }


  /**
   * Der Ring: Boden der Bühne, mit den Fenstern gerollt und gekippt. Die Teilstriche stehen
   * still (HUM-412).
   */
  function paintRing(G: Params, m: number, tIntro: number, ds: number[]) {
    const { lay } = s;
    const R = G.r * lay.Ws;
    const ringScale = (R + 0.06 * lay.Ws) / (lay.stage.r * lay.Ws + 0.06 * lay.Ws);
    // Die Fenster rollen nur zum Teil mit dem Bogen: ihre untere Ecke senkt sich unter die
    // Bodenlinie. Der Ring liegt um diese Senkung tiefer.
    const rho0 = lerp(lay.hero.rho, lay.stage.rho, smoothstep(clamp(m / 0.88)));
    const rz0 = Math.abs((1 - rho0) * G.beta) * RAD;
    const dip = (lay.Ws / 2) * Math.sin(rz0) + (lay.H / 2) * (1 - Math.cos(rz0));
    put(
      refs.ring,
      "transform",
      `translate3d(-50%, -50%, 0) translate3d(0, ${(lay.H / 2 + lay.T + 2 + dip).toFixed(1)}px, ${(-R).toFixed(1)}px) rotateX(90deg) scale(${ringScale.toFixed(4)})`,
    );
    const ringIn = s.reduced ? 1 : s.introT0 < 0 ? 0 : clamp((tIntro - 500) / 1200);
    // Telefon quer und hoch: der flach gesehene Ring liefe quer über die Beschriftung.
    const ringStage = lay.cls === "G" || lay.stack ? 0 : 0.7;
    const ringHero = lay.side || lay.stack ? 0 : 0.32;
    put(
      refs.ring,
      "opacity",
      (ringIn * lerp(ringHero, ringStage, smoothstep(map(m, 0.3, 0.9)))).toFixed(3),
    );
    const nearest = Math.min(...ds.map((d) => Math.abs(d)));
    put(refs.pool, "opacity", (smoothstep(map(m, 0.5, 1)) * (1 - clamp(nearest))).toFixed(3));
    put(
      refs.spot,
      "transform",
      `translate3d(${(0.6 * G.cx).toFixed(1)}px, ${(0.4 * G.cy).toFixed(1)}px, 0)`,
    );
    put(refs.spot, "opacity", lerp(1, 0.8, m).toFixed(3));
  }

  /** Überschrift und Lede gehen schneller als das Rig nach oben: Kran-Parallaxe. */
  function paintHero(m: number) {
    put(refs.heroText, "opacity", (1 - easeOutQuad(map(m, 0, 0.38))).toFixed(3));
    put(
      refs.heroText,
      "transform",
      `translate3d(0, ${(-0.12 * s.vh * easeInCubic(map(m, 0, 0.6))).toFixed(1)}px, 0)`,
    );
    const ctaOut = map(m, 0, 0.22);
    put(refs.actions, "opacity", (1 - ctaOut).toFixed(3));
    put(refs.actions, "transform", `translate3d(0, ${(16 * ctaOut).toFixed(1)}px, 0)`);
    put(refs.actions, "visibility", m > 0.25 ? "hidden" : "");
    // Der Hinweis geht als Erstes: er hat seine Arbeit getan, sobald jemand scrollt.
    if (refs.hint) {
      put(refs.hint, "opacity", (1 - map(m, 0, 0.1)).toFixed(3));
      put(refs.hint, "visibility", m > 0.12 ? "hidden" : "");
    }
  }

  /** Steht der Punkt (Viewport) über einem sichtbaren Fenster? Aus den Posen des letzten Bildes. */
  function overWindow(x: number, y: number): boolean {
    if (!lastG) return false;
    const G = lastG;
    const v = view();
    return lastPoses.some((p, i) => {
      if (lastOps[i] < 0.05) return false;
      const q = [-1, 1].flatMap((sx) =>
        [-1, 1].map((sy) =>
          project(v, toWorld(G, winPoint(p, (sx * s.lay.Ws) / 2, (sy * s.lay.H) / 2, 0, s.lay.H))),
        ),
      );
      const xs = q.map((c) => c[0]);
      const ys = q.map((c) => c[1]);
      return x >= Math.min(...xs) && x <= Math.max(...xs) && y >= Math.min(...ys) && y <= Math.max(...ys);
    });
  }

  return {
    render,
    read,
    setMorph,
    onScreen,
    stageY,
    overWindow,
    beta: () => (lastG ? lastG.beta : s.lay.stage.beta),
  };
}

export type Frame = ReturnType<typeof createFrame>;
