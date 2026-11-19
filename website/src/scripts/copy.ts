// Kopieren-Knöpfe (data-copy): legen genau den Text des Attributs in die Zwischenablage, etwa den
// Befehl, nie den Inhalt einer Seite. Eine eigene Datei, weil die Content-Security-Policy keine
// Inline-Skripte erlaubt; Download-Seite, Startseite und Leseseite des Skripts binden sie ein.

document.querySelectorAll<HTMLButtonElement>('[data-copy]').forEach((button) => {
  button.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(button.dataset.copy ?? '');
      button.toggleAttribute('data-copied', true);
      setTimeout(() => button.removeAttribute('data-copied'), 1600);
    } catch {
      // Ohne Erlaubnis für die Zwischenablage bleibt der Befehl zum Markieren stehen.
    }
  });
});
