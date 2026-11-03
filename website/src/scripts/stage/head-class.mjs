// Das blockierende Skript im Kopf der Startseite (HUM-418), gebaut aus classes.mjs: dieselben
// Grenzen und derselbe Quelltext von classify, damit Kopf und Bühne nie verschieden einteilen.
// src/pages/stage-class.js.ts liefert es als /stage-class.js aus; die CSP verbietet Inline-Skripte.

import { BOUNDS, H_BOUNDS, classify, heightMarks, ratio } from "./classes.mjs";

/**
 * Quelltext von /stage-class.js. Er misst einmal, bevor der Körper gelesen ist: innerWidth und
 * innerHeight, beim Laden am Telefon mit eingeblendeter Adressleiste, also die kleine Höhe, mit
 * der auch die Bühne rechnet (100svh). Danach teilt nur layout.ts neu ein, und nur, wenn sich die
 * Breite ändert.
 * @returns {string}
 */
export function headClassScript() {
  return `// Erzeugt aus src/scripts/stage/classes.mjs (HUM-418), nicht von Hand ändern.
(function () {
  var BOUNDS = ${JSON.stringify(BOUNDS)};
  var H_BOUNDS = ${JSON.stringify(H_BOUNDS)};
  var ratio = ${ratio.toString()};
  var classify = ${classify.toString()};
  var heightMarks = ${heightMarks.toString()};
  var root = document.documentElement;
  var w = window.innerWidth;
  var h = window.innerHeight;
  if (!w || !h) return;
  root.setAttribute("data-cls", classify(w, h));
  root.setAttribute("data-cls-w", String(w));
  heightMarks(h).forEach(function (b) {
    root.setAttribute("data-h" + b, "");
  });
})();
`;
}
