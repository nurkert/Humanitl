// Wache über das Video im vorderen Fenster (HUM-415): Entscheidungen ohne DOM, damit die Tests
// sie ohne Browser und Übersetzer laden (wie follow.mjs und gesture.mjs).
//
// Warum: Auf einem Telefon zeigte das vordere Fenster kurz das Video und blieb dann grau stehen
// (Rückmeldung des Inhabers vom 2026-11-02, nach HUM-412). Das Skript verließ sich darauf, dass
// der Browser ein Anhalten meldet (pause, waiting, stalled). Gibt der Browser aber den Decoder
// frei, ohne ein Ereignis zu schicken, oder malt er eine leere Fläche, während seine Uhr
// weiterläuft, blieb das Video sichtbar. Darum gilt jetzt: Das Video deckt das Bild nur, solange
// neue Bilder nachweislich kommen und nicht leer sind. Bleiben sie aus, steht wieder das Bild,
// egal was der Browser meldet. Wiederholt sich das, bleibt das Bild stehen.

/**
 * Ohne Frame-Callback (requestVideoFrameCallback fehlt) zählt die Uhr des Elements; steht sie so
 * lange in ms, deckt das Video das Bild nicht mehr.
 */
export const FREEZE_MS = 500;

/**
 * Mit Frame-Callback zählen nur gemalte Bilder, nie die Uhr: Eine Uhr kann weiterlaufen, während
 * der Browser nichts mehr malt. Die Aufnahme ist ein fast stehender Bildschirm, und Browser melden
 * ihre Bilder in Lücken: gemessen bis 580 ms in headless Firefox unter Last. Darum 900 ms.
 */
export const FRAME_FREEZE_MS = 900;

/**
 * Beim eigenen Sprung an den Anfang der Schleife steht das letzte Bild, bis das neue gemalt ist;
 * so lange in ms darf das dauern, dann gilt auch das als Stillstand.
 */
export const SEEK_GRACE_MS = 1500;

/**
 * Lagen zwischen zwei Prüfungen mehr als so viele ms, hing die Seite selbst (Laden, schwaches
 * Telefon) oder war eingefroren: In der Lücke konnte auch kein Bild gemeldet werden. Diese
 * Prüfung urteilt nicht; die nächste, nach der die Rückrufe der Lücke gelaufen sind, schon, auch
 * wenn sie ebenfalls spät kommt: Entschuldigt ist nur die erste Prüfung nach einer Lücke, sonst
 * urteilte eine sichtbare Seite mit gedrosselten Zeitgebern nie. Die Wache prüft alle 250 ms über
 * einen Zeitgeber und zusätzlich je Bild der Seite.
 */
export const GAP_MS = 1200;

/** Takt des Zeitgebers der Wache in ms; er hängt nicht an requestAnimationFrame. */
export const WATCH_TICK_MS = 250;

/** Abstand der Stichproben in ms, vor dem Öffnen wie danach. */
export const SAMPLE_MS = 500;

/** So viele leere oder nicht prüfbare Stichproben in Folge vor dem Öffnen: Video anhalten. */
export const PRE_OPEN_LIMIT = 3;

/** So viele leere oder nicht prüfbare Stichproben in Folge bei laufendem Video: Bild zeigen. */
export const ON_LIMIT = 2;

/** So oft in FLAP_WINDOW_MS darf das Video ausfallen, dann wartet es auf eine Geste. */
export const FLAP_LIMIT = 3;
export const FLAP_WINDOW_MS = 30000;

/** Nach so vielen Wartephasen bleibt für den Rest des Besuchs das Bild. */
export const PARK_LIMIT = 2;

/**
 * Urteil einer Prüfung über den Fortschritt des Videos, nach Uhrzeit (performance.now).
 * "skip": Seite unsichtbar oder die Prüfung kommt nach einer Lücke, kein Urteil.
 * @param {{ now: number, lastCheck: number, lastProgress: number, visible: boolean,
 *   frames: boolean, plannedSeek: boolean, afterGap?: boolean }} s
 *   afterGap: die vorige Prüfung wurde wegen einer Lücke übersprungen.
 *   lastProgress ist mit Frame-Callback der Zeitpunkt des letzten gemalten Bildes mit neuer
 *   Medienzeit, sonst der des letzten Fortschritts der Uhr.
 * @returns {"ok" | "frozen" | "skip"}
 */
export function judgeFreeze(s) {
  if (!s.visible || (s.now - s.lastCheck > GAP_MS && !s.afterGap)) return "skip";
  const limit = s.plannedSeek ? SEEK_GRACE_MS : s.frames ? FRAME_FREEZE_MS : FREEZE_MS;
  return s.now - s.lastProgress > limit ? "frozen" : "ok";
}

/**
 * Ist ein verkleinertes Bild des Videos leer? Eine einheitliche Fläche (grau, schwarz) oder
 * durchsichtige Pixel bedeuten: Der Browser liefert kein echtes Bild der Aufnahme. Die Aufnahme
 * selbst hat in jedem Bild helle Schrift auf dunklem Grund.
 * @param {ArrayLike<number>} rgba Pixel als RGBA, wie getImageData sie liefert
 * @returns {boolean}
 */
export function isBlankFrame(rgba) {
  const n = Math.floor(rgba.length / 4);
  if (n === 0) return true;
  let lo = 255;
  let hi = 0;
  let clear = 0;
  for (let i = 0; i < n; i++) {
    const a = rgba[i * 4 + 3];
    if (a < 250) clear++;
    const y =
      0.2126 * rgba[i * 4] + 0.7152 * rgba[i * 4 + 1] + 0.0722 * rgba[i * 4 + 2];
    if (y < lo) lo = y;
    if (y > hi) hi = y;
  }
  return clear * 2 > n || hi - lo < 8;
}

/**
 * Bewertet Stichproben: "ok" (echtes Bild), "blank" (leer) oder "unverified" (Stichprobe nicht
 * möglich: Canvas fehlt, drawImage oder getImageData wirft). Nicht prüfbar zählt wie leer: Ohne
 * Nachweis bleibt das Bild, schlimmstenfalls ohne Video, nie grau.
 */
export function createSampleJudge() {
  let pre = 0;
  let on = 0;
  return {
    /**
     * Vor dem Öffnen: "open" bei echtem Bild, "wait" bei leerem oder nicht prüfbarem, "park"
     * nach PRE_OPEN_LIMIT solchen in Folge (Video anhalten, kein endloses unsichtbares Dekodieren).
     * @param {"ok" | "blank" | "unverified"} v
     * @returns {"open" | "wait" | "park"}
     */
    preOpen(v) {
      if (v === "ok") {
        pre = 0;
        return "open";
      }
      pre++;
      if (pre < PRE_OPEN_LIMIT) return "wait";
      pre = 0;
      return "park";
    },
    /**
     * Bei laufendem Video: "fail" nach ON_LIMIT leeren oder nicht prüfbaren in Folge; ein
     * einzelnes Bild, das verkleinert leer wirkt, nimmt das Video nicht zurück.
     * @param {"ok" | "blank" | "unverified"} v
     * @returns {"ok" | "fail"}
     */
    whileOn(v) {
      if (v === "ok") {
        on = 0;
        return "ok";
      }
      on++;
      if (on < ON_LIMIT) return "ok";
      on = 0;
      return "fail";
    },
    reset() {
      pre = 0;
      on = 0;
    },
  };
}

/**
 * Zählt Ausfälle und entscheidet, ob das Video es noch einmal versucht.
 * "retry": neu anlaufen; "park": bis zur nächsten Geste das Bild; "stop": für den Besuch das Bild.
 * @returns {{ note(now: number): "retry" | "park" | "stop", park(): "park" | "stop",
 *   gesture(): void }}
 */
export function createFlapGuard() {
  /** @type {number[]} */
  let fails = [];
  let parks = 0;
  /** @returns {"park" | "stop"} */
  const park = () => {
    fails = [];
    parks++;
    return parks >= PARK_LIMIT ? "stop" : "park";
  };
  return {
    note(now) {
      fails = fails.filter((t) => now - t < FLAP_WINDOW_MS);
      fails.push(now);
      if (fails.length < FLAP_LIMIT) return "retry";
      return park();
    },
    /** Direkt anhalten (etwa nach leeren Stichproben vor dem Öffnen); zählt als Wartephase. */
    park,
    gesture() {
      fails = [];
    },
  };
}
