// Der Ablauf im vorderen Fenster der Bühne (HUM-389, HUM-412).
//
// Unter dem Video liegt immer die Aufnahme als Bild. Das Video ist durchsichtig, bis es
// nachweislich läuft (zwei Bilder mit fortschreitender Medienzeit), und wird wieder
// durchsichtig, sobald es das nicht mehr sicher tut: beim Anhalten, beim Warten auf Daten,
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

/** Ohne Fortschritt im Abspielen gilt das Video nach dieser Zeit als hängend. */
const STALL_MS = 2500;

/**
 * Hier beginnt die Schleife, in Sekunden: das erste Bild mit der gehaltenen Anfrage an
 * paste.rs liegt bei 2,1 s (aus den Einzelbildern der Aufnahme gemessen), davor steht die
 * leere Warteschlange.
 */
export const LOOP_START = 2.2;

/** Ab diesem Rücksprung der Medienzeit in s zeigt das Fenster wieder das Bild. */
const ROLLBACK_S = 0.25;

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

  const want = () =>
    !failed && !blocked && !reduced && !document.hidden && seen && shown;

  function setOn(on: boolean) {
    video.toggleAttribute("data-on", on);
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
    video.hidden = true;
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
        !video.paused
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
  function armGesture() {
    if (gestureArmed) return;
    gestureArmed = true;
    const retry = () => {
      gestureArmed = false;
      for (const type of GESTURES)
        document.removeEventListener(type, retry, true);
      blocked = false;
      // Eine echte Geste gibt dem Browser einen frischen Rahmen für eigene Pausen.
      pausesByBrowser = [];
      sync();
    };
    for (const type of GESTURES)
      document.addEventListener(type, retry, { capture: true, passive: true });
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
    lastProgress = performance.now();
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
