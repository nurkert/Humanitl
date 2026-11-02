// Der Hinweis unter den Knöpfen der Startseite (HUM-413): ein ruhiger Knopf, der weich bis
// dorthin scrollt, wo das Karussell fertig steht. Ohne Skript ist er ein Anker auf die
// Beschriftung; unter reduzierter Bewegung springt er, statt zu gleiten.

import type { Ctx } from "./context";

export function bindHint(ctx: Ctx, stageY: () => number) {
  const { hint } = ctx.refs;
  if (!hint) return;
  hint.addEventListener("click", (event) => {
    // Mittlere Taste und Klicks mit Zusatztaste öffnen den Anker wie gewohnt.
    if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey)
      return;
    event.preventDefault();
    const top = Math.round(stageY()) + 2;
    window.scrollTo({ top, behavior: ctx.s.reduced ? "auto" : "smooth" });
    // Der Fokus geht mit auf die Steuerung, sobald das Karussell bedienbar ist; so landen
    // Tastatur und Screenreader dort, wohin die Seite gescrollt hat. Langsame Geräte brauchen
    // für Scrollen und Morph länger, darum höchstens 4 s warten.
    const t0 = performance.now();
    const focus = () => {
      const next = ctx.refs.scene.querySelector<HTMLElement>("[data-next]");
      if (ctx.s.live && next) next.focus({ preventScroll: true });
      else if (performance.now() - t0 < 4000) window.setTimeout(focus, 100);
    };
    window.setTimeout(focus, 100);
  });
}
