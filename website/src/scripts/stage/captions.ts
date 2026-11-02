// Die Beschriftung des Karussells: zwei Schichten mit Wort-Masken, das Zählwerk und die Ansage
// für Screenreader (HUM-334, HUM-389, HUM-412).
//
// Seit HUM-413 erscheint die Beschriftung im Morph als eigene Bewegung in der Zeit, sobald
// der Morph die Bühne fast erreicht hat, statt Wort für Wort am Scrollweg zu hängen: Wer mitten
// im Morph aufhörte zu scrollen, sah den Fließtext halb ausgeblendet mitten in der Zeile
// stehen (Rückmeldung des Inhabers vom 2026-11-02).

import type { Ctx } from "./context";
import { modRing, wrapRing } from "./choreography.mjs";

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

const EASE_IN = "cubic-bezier(0.16, 1, 0.3, 1)";
const EASE_OUT = "cubic-bezier(0.55, 0, 0.75, 0.2)";

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

export function createCaptions(ctx: Ctx) {
  const { refs, s } = ctx;
  const { wins } = refs;
  const n = wins.length;
  const layers: Layer[] = refs.layerEls.map((el) => ({
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
  let shown = 0;
  let revealed = false;
  const changes: number[] = [];
  let burstTimer = 0;
  const phone = () => s.lay.cls === "F";

  // Die Beschriftung hält die Höhe der längsten Fassung frei, damit Index und Augenbraue
  // beim Wechsel nicht springen; gemessen an einer unsichtbaren Kopie einer Schicht.
  let probe: HTMLElement | null = null;
  function reserve() {
    if (!probe) {
      probe = refs.layerEls[0].cloneNode(true) as HTMLElement;
      probe.removeAttribute("data-layer");
      probe.classList.add("cap__probe");
      refs.layersBox.append(probe);
    }
    const fields = ["[data-cap-name]", "[data-cap-title]", "[data-cap-text]"].map(
      (q) => probe!.querySelector<HTMLElement>(q)!,
    );
    refs.layersBox.style.minHeight = "";
    let max = 0;
    for (const win of wins) {
      words(fields[0], win.dataset.name ?? "");
      words(fields[1], win.dataset.title ?? "");
      words(fields[2], win.dataset.text ?? "");
      max = Math.max(max, probe.offsetHeight);
    }
    refs.layersBox.style.minHeight = `${Math.ceil(max)}px`;
    odoEm = parseFloat(getComputedStyle(refs.odoStrip).fontSize) || 0;
  }

  function fillLayer(l: Layer, idx: number) {
    const win = wins[idx];
    for (const a of l.anims) a.cancel();
    l.anims = [];
    words(l.name, win.dataset.name ?? "");
    words(l.head, win.dataset.title ?? "");
    words(l.text, win.dataset.text ?? "");
    l.words = [...l.el.querySelectorAll<HTMLElement>(".wi")];
    l.group = l.words.map((w) => (l.name.contains(w) ? 0 : l.head.contains(w) ? 1 : 2));
    l.shown = idx;
    l.el.style.visibility = "";
    l.el.style.opacity = "";
    // Zeilen braucht nur die Beschriftung am Telefon; das Messen erzwingt ein Layout.
    if (phone()) measureLines(l);
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

  const hideLayer = (l: Layer) => {
    l.el.style.visibility = "hidden";
  };

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

  function animateIn(l: Layer, r: number, delay: number, fast = false) {
    clearInline(l);
    const onPhone = phone();
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
        : onPhone
          ? delay + l.line[i] * 60
          : delay + extra + k * (group === 2 ? bodyStep : 22);
      const from = onPhone
        ? `translate3d(${r * 24}px, 100%, 0)`
        : `translate3d(0, ${r * 110}%, 0) rotate(${r * 4}deg)`;
      l.anims.push(
        w.animate(
          [
            { transform: from, opacity: 0 },
            { transform: "translate3d(0, 0, 0) rotate(0deg)", opacity: 1 },
          ],
          {
            duration: fast ? 260 : onPhone ? 560 : 640,
            delay: at,
            easing: EASE_IN,
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
        easing: EASE_OUT,
        fill: "forwards",
      }),
    );
    l.words.forEach((w, j) => {
      l.anims.push(
        w.animate(
          [
            { transform: "translate3d(0, 0, 0)", opacity: 1 },
            { transform: `translate3d(0, ${-r * 105}%, 0)`, opacity: 0.2 },
          ],
          { duration: 220, delay: (last - j) * step, easing: EASE_OUT, fill: "forwards" },
        ),
      );
    });
    const token = l.shown;
    Promise.all(l.anims.map((x) => x.finished)).then(
      () => {
        if ((layers[front] !== l || !revealed) && l.shown === token) hideLayer(l);
      },
      () => undefined,
    );
  }

  // Die Ansage für Screenreader kommt erst, wenn der Ring steht, und nur bei einem anderen
  // Modul als zuletzt angesagt.
  let announced = -1;
  function announce(idx: number) {
    if (idx === announced) return;
    announced = idx;
    const win = wins[idx];
    refs.live.textContent = `${win.dataset.name ?? ""}. ${win.dataset.title ?? ""} ${win.dataset.text ?? ""}`;
  }

  function sweepStreak() {
    if (s.reduced) return;
    layers[front].streak.animate(
      [
        { transform: "translate3d(-100%, 0, 0)", opacity: 0 },
        { opacity: 0.9, offset: 0.35 },
        { transform: "translate3d(260%, 0, 0)", opacity: 0 },
      ],
      { duration: 900, easing: "ease-out" },
    );
  }

  /**
   * Wechsel der Beschriftung; r ist die Drehrichtung. Solange sie nicht gezeigt wird (im
   * Morph), tauscht nur der Inhalt.
   */
  function change(idx: number, r: number, settledNow: boolean) {
    if (settledNow) announce(idx);
    shown = idx;
    if (!revealed || s.reduced) {
      fillLayer(layers[front], idx);
      return;
    }
    const now = performance.now();
    changes.push(now);
    while (changes.length && now - changes[0] > 400) changes.shift();
    const burst = changes.length >= 3;
    const old = layers[front];
    const neu = layers[1 - front];
    settleAnims(old);
    settleAnims(neu);
    front = 1 - front;
    fillLayer(neu, idx);
    if (burst) {
      // Schnelles Drehen: Texte tauschen sofort ohne Ausgang, jeder Stand steigt gleich auf.
      hideLayer(old);
      animateIn(neu, r, 0, true);
      clearTimeout(burstTimer);
      burstTimer = window.setTimeout(sweepStreak, 160);
      return;
    }
    animateOut(old, r);
    animateIn(neu, r, 110);
    sweepStreak();
  }

  /**
   * Zeigt oder verbirgt die Beschriftung als Ganzes. Das CSS blendet Kopf und Kasten über
   * data-cap-in; die Wörter steigen über ihre eigenen Animationen auf.
   */
  function reveal(on: boolean, now = false) {
    // now: ohne Ausgang sofort weg (der Ring beginnt zurückzudrehen, Review HUM-413).
    refs.scene.toggleAttribute("data-cap-cut", !on && now);
    if (!on && now && !revealed) {
      settleAnims(layers[0]);
      settleAnims(layers[1]);
      hideLayer(layers[0]);
      hideLayer(layers[1]);
      return;
    }
    if (on === revealed) return;
    revealed = on;
    refs.scene.toggleAttribute("data-cap-in", on);
    const l = layers[front];
    settleAnims(layers[0]);
    settleAnims(layers[1]);
    clearTimeout(burstTimer);
    hideLayer(layers[1 - front]);
    if (on) {
      fillLayer(l, shown);
      if (!s.reduced) {
        animateIn(l, 1, 140);
        sweepStreak();
      }
    } else if (!s.reduced && !now) {
      animateOut(l, -1);
    } else hideLayer(l);
  }

  // ---- Zählwerk -------------------------------------------------------------------
  // Der Index als Zählwerk: zwei Ziffernspalten, die Einer laufen in Drehrichtung weiter.
  let odoQ = 1;
  let odoV = 0;
  let odoTarget = 1;
  let odoShown = 0;
  // Höhe einer Ziffer in px (1em der Spalte); reserve() setzt sie je Bildausschnitt.
  let odoEm = 0;

  function odoTo(idx: number, r: number) {
    const from = ((Math.round(odoTarget) % 10) + 10) % 10;
    const to = idx + 1;
    let dq: number;
    const jump = Math.abs(wrapRing(idx - odoShown, n));
    if (jump > 3 || r === 0) dq = ((((to - from) % 10) + 15) % 10) - 5;
    else if (r > 0) dq = (((to - from) % 10) + 10) % 10;
    else dq = -((((from - to) % 10) + 10) % 10);
    odoShown = idx;
    odoTarget += dq;
    if (s.reduced || r === 0) {
      odoQ = odoTarget;
      odoV = 0;
    }
  }

  /** Ein Schritt der Feder des Zählwerks; liefert true, solange es läuft. */
  function odoStep(h: number): boolean {
    if (odoQ === odoTarget) return false;
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

  function paintOdo() {
    const digit = ((odoQ % 10) + 10) % 10;
    // In Ruhe auf ganze Pixel: die Spalte ist eine eigene Ebene, ein Bruchteil ließe die
    // Konturziffern unscharf stehen.
    ctx.put(
      refs.odoStrip,
      "transform",
      odoQ === odoTarget
        ? `translate3d(0, ${-Math.round(digit * odoEm)}px, 0)`
        : `translate3d(0, ${(-digit).toFixed(4)}em, 0)`,
    );
  }

  /**
   * Springt ohne Übergang auf ein Modul (Rücksetzen oben im Stapel, unsichtbar). Die Ansage
   * folgt, damit ein Screenreader nicht das zuletzt gedrehte Modul vorliest.
   */
  function jump(idx: number) {
    shown = idx;
    announce(idx);
    fillLayer(layers[front], idx);
    hideLayer(layers[1 - front]);
    odoTo(idx, 0);
  }

  function remeasure() {
    if (phone()) for (const l of layers) if (l.words.length) measureLines(l);
  }

  function init(idx: number) {
    hideLayer(layers[1]);
    jump(idx);
    paintOdo();
  }

  return {
    reserve,
    init,
    change,
    reveal,
    jump,
    remeasure,
    announce,
    announceAt: (pos: number) => announce(modRing(Math.round(pos), n)),
    odoTo,
    odoStep,
    paintOdo,
    get shown() {
      return shown;
    },
    get revealed() {
      return revealed;
    },
  };
}

export type Captions = ReturnType<typeof createCaptions>;
