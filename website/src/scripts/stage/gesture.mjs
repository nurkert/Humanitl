// Gesten des Karussells: Achse einer Rad-Geste und Richtung beim Ziehen, beide mit Hysterese.
// Als .mjs mit JSDoc, damit die Tests es ohne Übersetzer laden (wie follow.mjs).
//
// Warum: Das Karussell entschied je einzelnem Rad-Ereignis, ob es waagrecht ist
// (|deltaX| > 1,2 · |deltaY|). Beim senkrechten Scrollen mit dem Trackpad rauscht deltaX, und
// gegen Ende des Nachlaufs sind beide Werte klein: Einzelne Ereignisse galten dann als
// waagrecht, drehten den Ring ein Stück, hielten die Seite an (preventDefault) und ließen ihn
// 140 ms später zurückfedern. Ebenso drehte jeder Wechsel im Vorzeichen eines Ereignisses die
// Zugrichtung und damit das Fenster mit Vortritt: Beim Zittern von Finger oder Trackpad sprang
// das vordere Fenster im Takt der Ereignisse vor und zurück.

/** Ohne Ereignis so lange gilt eine Rad-Geste als beendet, in ms. */
export const WHEEL_IDLE_MS = 160;

/**
 * Rad-Weg in Pixeln: Mäuse unter Firefox und Linux melden Zeilen (deltaMode 1), manche Geräte
 * Seiten (2). Die Achse wird erst nach der Umrechnung bestimmt, sonst schluckte die Schwelle
 * die ersten Rasten einer Zeilen-Maus.
 * @param {{ deltaX: number, deltaY: number, deltaMode: number }} event
 * @param {number} page Breite einer Seite in px
 * @returns {[number, number]}
 */
export function wheelPixels(event, page) {
  const unit = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? page : 1;
  return [event.deltaX * unit, event.deltaY * unit];
}

/**
 * Bestimmt die Achse einer Rad-Geste einmal an ihrem Anfang und hält sie bis zur Pause.
 * Liefert je Ereignis "x", "y" oder null, solange noch nicht entschieden ist.
 * @param {{ idleMs?: number, decidePx?: number, ratio?: number }} [options]
 */
export function createWheelAxis({ idleMs = WHEEL_IDLE_MS, decidePx = 8, ratio = 1.5 } = {}) {
  /** @type {"x" | "y" | null} */
  let axis = null;
  let last = -Infinity;
  let sx = 0;
  let sy = 0;
  /**
   * @param {number} dx
   * @param {number} dy
   * @param {number} now Zeit in ms
   * @returns {"x" | "y" | null}
   */
  return function classify(dx, dy, now) {
    if (now - last > idleMs) {
      axis = null;
      sx = 0;
      sy = 0;
    }
    last = now;
    if (axis) return axis;
    sx += Math.abs(dx);
    sy += Math.abs(dy);
    if (Math.max(sx, sy) < decidePx) return null;
    axis = sx > ratio * sy ? "x" : "y";
    return axis;
  };
}

/**
 * Richtung einer Bewegung mit Hysterese: Sie kippt erst, wenn die Gegenbewegung seit dem
 * letzten Schritt in Richtung mindestens threshold beträgt (in Fenstern).
 * @param {number} [threshold]
 */
export function createDirectionLatch(threshold = 0.06) {
  let dir = 1;
  let back = 0;
  return {
    /**
     * @param {number} delta Bewegung seit dem letzten Aufruf
     * @returns {number} 1 oder -1
     */
    push(delta) {
      if (delta === 0) return dir;
      if (Math.sign(delta) === dir) {
        back = 0;
        return dir;
      }
      back += Math.abs(delta);
      if (back >= threshold) {
        dir = -dir;
        back = 0;
      }
      return dir;
    },
    /** @param {number} d */
    reset(d) {
      dir = d >= 0 ? 1 : -1;
      back = 0;
    },
    get dir() {
      return dir;
    },
  };
}
