// Wie oft eine Installationsdatei zählt (#104). GitHub zählt jeden Abruf, auch automatische
// und wiederholte; eine einzelne Datei, die in einer Schleife geladen wird, würde sonst die
// ganze Zahl der Seite bestimmen. Deshalb zählt jede Datei höchstens bis zu einer Obergrenze,
// die aus den Zahlen selbst kommt: dem Fünffachen des Medians aller Dateien mit mindestens
// einem Abruf, aber nie weniger als `FLOOR`. Eine Datei unter der Grenze zählt, wie sie
// ausgeliefert wurde.

/** Untergrenze der Obergrenze: Bei wenigen, kleinen Zahlen wird nichts gekappt. */
export const FLOOR = 25;
/** So viele Mediane darf eine einzelne Datei erreichen, bevor sie gekappt wird. */
export const FACTOR = 5;

/** Median einer Liste von Zahlen; 0 für eine leere Liste. */
export function median(values) {
  if (values.length === 0) return 0;
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
}

/** Die Obergrenze für die gegebenen Rohzahlen aller Installationsdateien. */
export function countCap(counts) {
  const positive = counts.filter((count) => count > 0);
  return Math.max(FLOOR, Math.ceil(FACTOR * median(positive)));
}

/** Die Rohzahlen, jede auf die gemeinsame Obergrenze gekappt; Reihenfolge bleibt. */
export function dampCounts(counts) {
  const cap = countCap(counts);
  return counts.map((count) => Math.min(count, cap));
}
