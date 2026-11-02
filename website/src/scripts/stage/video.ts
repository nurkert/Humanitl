// Der Ablauf im vorderen Fenster der Bühne (HUM-389, HUM-412, HUM-415).
//
// Über dem Video liegt immer die Aufnahme als Bild. Das Bild wird erst durchsichtig, wenn das
// Video nachweislich läuft (zwei Bilder mit fortschreitender Medienzeit), und deckt es wieder,
// sobald es das nicht mehr sicher tut: beim Anhalten, beim Warten auf Daten,
// beim Hängen, bei einem Rücksprung der Zeit, den das Skript nicht selbst ausgelöst hat, wenn
// der Browser den Decoder freigibt oder das Abspielen verweigert. So kann das Fenster nie leer
// sein; schlimmstenfalls steht das Bild.
//
// Die Schleife beginnt nicht bei 0: Die ersten gut zwei Sekunden der Aufnahme zeigen die fast
// leere Warteschlange („The agent is working“), auf einem kleinen Fenster eine leere Fläche.
// Das Skript startet darum bei LOOP_START und springt am Ende dorthin zurück; das Attribut
// loop fehlt im Markup.
//
// Warum so streng: Auf Telefonen stand das Fenster nach kurzer Zeit leer (Rückmeldung des
// Inhabers vom 2026-11-02). Das alte Skript rief play() auf und verwarf den Ausgang. Ein Video,
// dessen Abspielen verweigert wird (Stromsparmodus), das der Browser selbst anhält oder das
// beim Nachladen hängt, blieb sichtbar stehen: auf seinem ersten Bild, der fast leeren
// Warteschlange, oder schwarz, wenn der Browser Puffer und Decoder zurückgeholt hatte. Dazu
// kehrte die Schleife alle 16,5 s auf eben dieses erste Bild zurück.
//
// HUM-415: Trotzdem stand das Fenster auf einem Telefon nach kurzem Video grau. Zwei Lücken:
// Die Deckkraft des Videos war die einzige Sperre, und ein Browser, der das Video in einer
// eigenen Ebene malt, muss sie dort nicht befolgen; und ohne Ereignis des Browsers merkte das
// Skript einen Stillstand erst nach STALL_MS. Jetzt liegt das Bild ÜBER dem Video und wird nur
// durchsichtig, solange das Video läuft (data-video-on am Rahmen). Eine Wache (Zeitgeber alle
// 250 ms und je Bild der Seite) nimmt das Video ohne neues gemaltes Bild zurück, mit
// Frame-Callback nach FRAME_FREEZE_MS, sonst nach FREEZE_MS. Stichproben alle 500 ms prüfen, ob
// das gelieferte Bild leer ist, auch vor dem Öffnen; eine nicht mögliche Stichprobe zählt wie
// ein leeres Bild. Wiederholte Ausfälle lassen das Bild stehen (stage/watch.mjs).

import {
  SAMPLE_MS,
  WATCH_TICK_MS,
  createFlapGuard,
  createSampleJudge,
  isBlankFrame,
  judgeFreeze,
} from "./watch.mjs";

/** Ohne Fortschritt im Abspielen gilt das Video nach dieser Zeit als hängend. */
const STALL_MS = 2500;

/**
 * Hier beginnt die Schleife, in Sekunden: das erste Bild mit der gehaltenen Anfrage an
 * paste.rs liegt bei 2,1 s (aus den Einzelbildern der Aufnahme gemessen), davor steht die
 * leere Warteschlange.
 */
export const LOOP_START = 2.2;

/**
 * Ab diesem Rücksprung der Medienzeit in s zeigt das Fenster wieder das Bild. Firefox meldet im
 * Frame-Callback gelegentlich 0,27 s zurück, ohne dass etwas geschieht (HUM-415); ein echtes
 * Zurückfallen (Neuladen, Anfang) springt Sekunden.
 */
const ROLLBACK_S = 0.5;

/** Seitenlänge der Stichprobe in Pixeln; feine Schrift überlebt 32x18 mit Glättung. */
const SAMPLE_W = 32;
const SAMPLE_H = 18;

type Sample = "ok" | "blank" | "unverified";

export interface FrontVideo {
  /** Sein Fenster ist nah am Fokus und sichtbar. */
  setShown(shown: boolean): void;
  /** Die Bühne schneidet den Viewport. */
  setSeen(seen: boolean): void;
  setReduced(reduced: boolean): void;
  /** Nach Sichtbarkeitswechsel des Dokuments erneut entscheiden. */
  sync(): void;
}

const NONE: FrontVideo = {
  setShown: () => undefined,
  setSeen: () => undefined,
  setReduced: () => undefined,
  sync: () => undefined,
};

type FrameVideo = HTMLVideoElement & {
  requestVideoFrameCallback?: (
    cb: (now: number, meta?: { mediaTime?: number }) => void,
  ) => number;
  cancelVideoFrameCallback?: (handle: number) => void;
};

export function frontVideo(
  el: HTMLVideoElement | null,
  initiallyReduced: boolean,
): FrontVideo {
  if (!el) return NONE;
  const video = el as FrameVideo;
  // Mit Frame-Callback zählt nur die Medienzeit gemalter Bilder; currentTime läuft ihr oft
  // voraus, gemischt sähe das wie ein Rücksprung aus. Ohne ihn zählt timeupdate.
  const frames = typeof video.requestVideoFrameCallback === "function";
  let shown = false;
  let seen = false;
  let reduced = initiallyReduced;
  // Endgültig ausgefallen (keine Quelle, Decoder-Fehler): das Bild bleibt, das Video geht.
  let failed = false;
  // Abspielen ohne Geste verweigert: erst die nächste Berührung oder Taste versucht es neu.
  let blocked = false;
  // Der Browser hat Puffer und Decoder verworfen; vor dem nächsten Abspielen neu laden.
  let emptied = false;
  // Sperre: das Video deckt das Bild erst nach zwei Fortschritten der Medienzeit.
  let gated = true;
  let advances = 0;
  let lastTime = -1;
  // Ein Rücksprung, den das Skript selbst auslöst (Schleife, Start bei LOOP_START).
  let plannedSeek = false;
  let lastProgress = 0;
  let stalls = 0;
  let watchdog = 0;
  let frameHandle = 0;
  let playToken = 0;
  // Das Skript hält selbst an; jedes andere pause kommt vom Browser.
  let selfPause = false;
  let selfLoad = false;
  // Hält der Browser das Video immer wieder an, wartet es auf eine Geste statt zu kämpfen.
  let pausesByBrowser: number[] = [];
  // Ausfälle der Wache (Stillstand ohne Ereignis, leere Bilder) gegen Flackern.
  const flaps = createFlapGuard();
  const samples = createSampleJudge();
  let guardFrame = 0;
  let guardTimer = 0;
  let lastCheck = 0;
  // Die vorige Prüfung kam nach einer Lücke und urteilte nicht; die nächste muss urteilen.
  let afterGap = false;
  let lastSample = 0;
  let lastPreSample = -Infinity;
  let sampler: CanvasRenderingContext2D | null = null;
  // Der Rahmen trägt das Bild; ist das Video an, wird nur das Bild durchsichtig.
  const frame = video.parentElement;

  const want = () =>
    !failed && !blocked && !reduced && !document.hidden && seen && shown;

  function setOn(on: boolean) {
    video.toggleAttribute("data-on", on);
    frame?.toggleAttribute("data-video-on", on);
    if (on && !guardTimer) {
      lastSample = lastCheck = performance.now();
      afterGap = false;
      samples.reset();
      // Der Zeitgeber hängt nicht an requestAnimationFrame: Drosselt der Browser die Bilder der
      // Seite (Stromsparen) oder hält er sie an, prüft die Wache trotzdem.
      guardTimer = window.setInterval(check, WATCH_TICK_MS);
      guardFrame = requestAnimationFrame(frameTick);
    } else if (!on && guardTimer) {
      clearInterval(guardTimer);
      cancelAnimationFrame(guardFrame);
      guardTimer = guardFrame = 0;
    }
  }

  function frameTick() {
    guardFrame = requestAnimationFrame(frameTick);
    check();
  }

  /**
   * Die Wache, solange das Video das Bild deckt: Ohne neues gemaltes Bild (FRAME_FREEZE_MS, ohne
   * Frame-Callback die Uhr nach FREEZE_MS) oder mit zwei leeren beziehungsweise nicht prüfbaren
   * Stichproben in Folge steht wieder das Bild, gleich ob der Browser ein Ereignis gemeldet hat.
   * Nach einer Lücke zwischen zwei Prüfungen (die Seite hing) urteilt erst die nächste.
   */
  function check() {
    if (!guardTimer) return;
    const now = performance.now();
    // Ohne Frame-Callback die Medienzeit selbst ablesen; timeupdate kommt nur alle 250 ms.
    if (!frames && !video.paused) progress(video.currentTime);
    if (!guardTimer) return;
    const verdict = judgeFreeze({
      now,
      lastCheck,
      lastProgress,
      visible: !document.hidden,
      frames,
      plannedSeek,
      afterGap,
    });
    afterGap = verdict === "skip" && !document.hidden;
    lastCheck = now;
    if (verdict === "frozen") {
      fail(now);
      return;
    }
    if (verdict === "ok" && now - lastSample >= SAMPLE_MS) {
      lastSample = now;
      if (samples.whileOn(sample()) === "fail") fail(now);
    }
  }

  /** Ein verkleinertes Bild des Videos: echt, leer, oder nicht prüfbar. */
  function sample(): Sample {
    if (video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) return "unverified";
    try {
      if (!sampler) {
        const canvas = document.createElement("canvas");
        canvas.width = SAMPLE_W;
        canvas.height = SAMPLE_H;
        sampler = canvas.getContext("2d", { willReadFrequently: true });
        if (!sampler) return "unverified";
        sampler.imageSmoothingEnabled = true;
        sampler.imageSmoothingQuality = "high";
      }
      sampler.drawImage(video, 0, 0, SAMPLE_W, SAMPLE_H);
      return isBlankFrame(sampler.getImageData(0, 0, SAMPLE_W, SAMPLE_H).data)
        ? "blank"
        : "ok";
    } catch {
      return "unverified";
    }
  }

  /**
   * Darf das Gate öffnen? Höchstens eine Stichprobe je SAMPLE_MS; ohne echtes Bild bleibt die
   * Aufnahme deckend, und nach mehreren leeren oder nicht prüfbaren Stichproben in Folge hält
   * das Video an, statt unsichtbar weiter zu dekodieren.
   */
  function mayOpen(): boolean {
    const now = performance.now();
    if (now - lastPreSample < SAMPLE_MS) return false;
    lastPreSample = now;
    const verdict = samples.preOpen(sample());
    if (verdict === "open") return true;
    if (verdict === "park") park(flaps.park());
    return false;
  }

  /** Die Wache hat einen Ausfall gesehen: Bild zeigen, beim Wiederholen dabei bleiben. */
  function fail(now: number) {
    gate();
    const verdict = flaps.note(now);
    if (verdict !== "retry") park(verdict);
  }

  /** Bis zur nächsten Geste ("park") oder für den Besuch ("stop") nur das Bild. */
  function park(verdict: "park" | "stop") {
    if (verdict === "stop") {
      drop();
      return;
    }
    blocked = true;
    playToken++;
    gate();
    stopWatch();
    if (!video.paused) {
      selfPause = true;
      video.pause();
    }
    armGesture();
  }

  /** Bild zeigen und auf frischen Fortschritt warten. */
  function gate() {
    gated = true;
    advances = 0;
    setOn(false);
  }

  function drop() {
    failed = true;
    gate();
    stopWatch();
    // Ein ausgefallenes Video dekodiert nicht unsichtbar weiter.
    if (!video.paused) {
      selfPause = true;
      video.pause();
    }
    video.hidden = true;
    // Ausgefallen ist endgültig: keine Geste startet es neu.
    disarmGesture();
  }

  function stopWatch() {
    clearTimeout(watchdog);
    watchdog = 0;
    if (frameHandle && video.cancelVideoFrameCallback)
      video.cancelVideoFrameCallback(frameHandle);
    frameHandle = 0;
  }

  /** Springt zum Anfang der Schleife; der Rücksprung zählt nicht als Stocken. */
  function seekLoopStart() {
    plannedSeek = true;
    try {
      video.currentTime = LOOP_START;
    } catch {
      /* ohne Metadaten gilt die Zeit als Startposition, sobald sie da sind (loadedmetadata) */
    }
  }

  /** Ein beobachteter Stand der Medienzeit, aus einem gemalten Bild oder aus timeupdate. */
  function progress(t: number) {
    if (t > lastTime) {
      lastTime = t;
      lastProgress = performance.now();
      stalls = 0;
      advances++;
      if (
        gated &&
        advances >= 2 &&
        video.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA &&
        want() &&
        !video.paused &&
        // Ein leeres oder nicht prüfbares Bild deckt die Aufnahme gar nicht erst auf.
        mayOpen()
      ) {
        gated = false;
        setOn(true);
      }
      return;
    }
    // Kleines Zurück (ein doppelt gemeldetes Bild, Zittern der Uhr) ist kein Rücksprung.
    if (t < lastTime - ROLLBACK_S) {
      // Der eigene Sprung zum Schleifenanfang: das letzte Bild steht, bis das neue gemalt ist.
      if (plannedSeek && Math.abs(t - LOOP_START) < 0.5) {
        plannedSeek = false;
        lastTime = t;
        lastProgress = performance.now();
        return;
      }
      // Jeder andere Rücksprung (Neuladen, Rückfall auf den Anfang): neu prüfen.
      lastTime = t;
      gate();
    }
  }

  function watchFrames() {
    if (!frames || frameHandle) return;
    const onFrame = (_now: number, meta?: { mediaTime?: number }) => {
      frameHandle = 0;
      if (video.paused) return;
      progress(meta?.mediaTime ?? video.currentTime);
      frameHandle = video.requestVideoFrameCallback!(onFrame);
    };
    frameHandle = video.requestVideoFrameCallback!(onFrame);
  }

  /** Läuft das Video, aber die Zeit steht, hängt es: Bild zeigen und neu anlaufen. */
  function watch() {
    clearTimeout(watchdog);
    watchdog = window.setTimeout(() => {
      watchdog = 0;
      if (!want() || video.paused) return;
      // Jeder weitere Anlauf ohne Fortschritt wartet doppelt so lange, höchstens 20 s.
      const limit = STALL_MS * 2 ** Math.min(stalls, 3);
      if (performance.now() - lastProgress >= limit) {
        stalls++;
        gate();
        // Ohne aktuelles Bild hat der Browser den Puffer verworfen: vor dem Anlauf neu laden.
        emptied ||= video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA;
        selfPause = true;
        video.pause();
        start();
        return;
      }
      watch();
    }, STALL_MS / 2);
  }

  function start() {
    if (emptied) {
      emptied = false;
      // Unser eigenes load() meldet emptied; das ist kein neuer Verlust.
      selfLoad = video.networkState !== HTMLMediaElement.NETWORK_EMPTY;
      video.load();
    }
    video.muted = true;
    gate();
    if (video.currentTime < LOOP_START - 0.05) seekLoopStart();
    lastProgress = performance.now();
    lastTime = video.currentTime;
    // Der Sprung eben ist schon eingerechnet; ein späterer Rücksprung zählt wieder.
    plannedSeek = false;
    const token = ++playToken;
    let p: Promise<void> | undefined;
    try {
      p = video.play();
    } catch {
      p = Promise.reject(new Error("play"));
    }
    watchFrames();
    watch();
    Promise.resolve(p).catch((error: unknown) => refused(error, token));
  }

  /** Ein abgelehntes play(), aus start() oder vom Neustart der Schleife. */
  function refused(error: unknown, token: number) {
    if (token !== playToken) return;
    gate();
    stopWatch();
    const name = (error as { name?: string } | null)?.name;
    // Ein späteres pause() bricht ein laufendes play() mit AbortError ab: kein Ausfall.
    if (name === "AbortError") return;
    if (name === "NotSupportedError") {
      drop();
      return;
    }
    // NotAllowedError und Unbekanntes: auf die nächste Geste warten.
    blocked = true;
    armGesture();
  }

  let gestureArmed = false;
  /** Hört nicht mehr auf die nächste Geste. */
  function disarmGesture() {
    if (!gestureArmed) return;
    gestureArmed = false;
    for (const type of GESTURES)
      document.removeEventListener(type, retryOnGesture, true);
  }

  function retryOnGesture() {
    disarmGesture();
    blocked = false;
    // Eine echte Geste gibt dem Browser einen frischen Rahmen für eigene Pausen.
    pausesByBrowser = [];
    flaps.gesture();
    sync();
  }

  function armGesture() {
    if (gestureArmed) return;
    gestureArmed = true;
    for (const type of GESTURES)
      document.addEventListener(type, retryOnGesture, {
        capture: true,
        passive: true,
      });
  }

  function sync() {
    if (failed) return;
    if (want()) {
      if (video.paused) start();
      return;
    }
    // Angehalten zeigt das Fenster wieder das Bild: ein angehaltenes Video darf der Browser
    // jederzeit entladen, und was es dann malt, ist nicht festgelegt.
    playToken++;
    stopWatch();
    gate();
    if (!video.paused) {
      selfPause = true;
      video.pause();
    }
    if (reduced) seekLoopStart();
  }

  if (!frames)
    video.addEventListener("timeupdate", () => {
      if (!video.paused) progress(video.currentTime);
    });
  video.addEventListener("playing", () => {
    // Nur für den Anlauf; deckt das Video schon, zählen allein gemalte Bilder (HUM-415).
    if (gated) lastProgress = performance.now();
    watchFrames();
  });
  // Warten auf Daten: das letzte Bild steht oder wird schwarz, je nach Browser. Bild zeigen,
  // bis wieder zwei Bilder fortschreiten.
  // Ausgenommen der eigene Sprung zum Schleifenanfang: Beim Suchen bleibt das letzte Bild
  // stehen, und das ist ein Bild vom Ende der Aufnahme, keine leere Fläche.
  const starve = () => {
    if (plannedSeek && video.seeking) return;
    if (
      !video.paused &&
      video.readyState < HTMLMediaElement.HAVE_FUTURE_DATA
    )
      gate();
  };
  video.addEventListener("waiting", starve);
  video.addEventListener("stalled", starve);
  // Die Schleife: am Ende zurück an ihren Anfang, das letzte Bild bleibt bis dahin stehen.
  video.addEventListener("ended", () => {
    if (!want()) return;
    seekLoopStart();
    const token = playToken;
    video.play().catch((error: unknown) => refused(error, token));
  });
  video.addEventListener("loadedmetadata", () => {
    if (video.currentTime < LOOP_START - 0.05) seekLoopStart();
  });
  // Vom Browser angehalten (Energiesparen, Audio-Fokus, Richtlinie für unsichtbare Videos):
  // Bild zeigen und neu anlaufen, beim dritten Mal in zehn Sekunden erst nach einer Geste. Das
  // Ende der Aufnahme hält das Element selbst an; das übernimmt ended.
  video.addEventListener("pause", () => {
    if (selfPause) {
      selfPause = false;
      return;
    }
    if (video.ended) return;
    gate();
    stopWatch();
    if (!want()) return;
    const now = performance.now();
    pausesByBrowser = pausesByBrowser.filter((t) => now - t < 10000);
    pausesByBrowser.push(now);
    if (pausesByBrowser.length >= 3) {
      blocked = true;
      armGesture();
      return;
    }
    window.setTimeout(sync, 0);
  });
  video.addEventListener("emptied", () => {
    if (selfLoad) {
      selfLoad = false;
      return;
    }
    emptied = true;
    gate();
  });
  video.addEventListener("error", () => {
    // Ein Fehler des Mediums (Decoder, Netz) ist endgültig: das Bild bleibt.
    if (video.error) drop();
  });
  // Die letzte Quelle fehlt oder ist kaputt; auch wenn beide schon gescheitert sind, bevor
  // dieses Skript lief.
  video
    .querySelector("source:last-of-type")
    ?.addEventListener("error", drop);
  if (
    video.networkState === HTMLMediaElement.NETWORK_NO_SOURCE &&
    !video.currentSrc
  )
    drop();

  return {
    setShown(v) {
      if (v === shown) return;
      shown = v;
      sync();
    },
    setSeen(v) {
      if (v === seen) return;
      seen = v;
      sync();
    },
    setReduced(v) {
      reduced = v;
      sync();
    },
    sync,
  };
}

const GESTURES = ["pointerdown", "touchend", "keydown"] as const;
