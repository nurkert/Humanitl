// Der Folger des Morphs: m läuft dem Scrollweg mRaw hinterher, ohne je zu springen.
//
// Vorher folgte m mit einer Zeitkonstante von 90 ms und sprang auf den Scrollweg, sobald der
// Abstand 0,5 überstieg oder der Scrollweg ein Ende erreichte, während m noch mehr als 0,2
// zurücklag. Ein gewöhnlicher Wurf auf dem Telefon (5000 px/s) oder ein schneller Zug am
// Trackpad löste das fast immer aus: Die Bühne sprang in einem Bild um bis zu 0,57 des Morphs.
// Jetzt ist der Folger eine kritisch gedämpfte Feder. Sie beschleunigt weich aus der Ruhe,
// überholt nicht und kommt bei jedem Tempo stetig an; springen darf nur, wer die Bühne nicht
// sieht (stage.ts).

// Als .mjs mit JSDoc, damit die Tests es ohne Übersetzer laden (wie src/lib/docs-catalog.mjs).

/**
 * @typedef {object} Follow
 * @property {number} m Morph, 0 bis 1.
 * @property {number} v Geschwindigkeit in Morph je Sekunde.
 */

/**
 * Eigenfrequenz in 1/s: Bei konstantem Scrollen liegt m um 2/ω Sekunden zurück (rund 130 ms,
 * so viel wie der alte Folger bei 90 ms Zeitkonstante plus Anlauf), aus der Ruhe ist ein Sprung
 * nach gut 300 ms zu 95 % eingeholt.
 */
export const FOLLOW_W = 15;

/**
 * Ein Schritt um dt Sekunden; liefert true, solange m sich noch bewegt.
 * @param {Follow} s
 * @param {number} goal
 * @param {number} dt
 * @param {number} [w]
 * @returns {boolean}
 */
export function followStep(s, goal, dt, w = FOLLOW_W) {
  let left = Math.min(dt, 0.1);
  // Feste kleine Schritte: die Feder bleibt auch bei 30 Bildern je Sekunde stabil und gleich.
  while (left > 1e-9) {
    const h = Math.min(left, 1 / 240);
    const a = w * w * (goal - s.m) - 2 * w * s.v;
    s.v += a * h;
    s.m += s.v * h;
    left -= h;
  }
  // Über die Enden hinaus gibt es keinen Morph; dort endet auch die Bewegung.
  if (s.m < 0 || s.m > 1) {
    s.m = Math.min(1, Math.max(0, s.m));
    s.v = 0;
  }
  if (Math.abs(goal - s.m) < 1e-4 && Math.abs(s.v) < 1e-3) {
    s.m = goal;
    s.v = 0;
    return false;
  }
  return true;
}
