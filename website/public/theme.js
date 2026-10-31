// Setzt eine gespeicherte Themenwahl vor dem ersten Bild (HUM-338). Die Datei liegt extern und
// lädt blockierend im Kopf, weil die CSP Inline-Skripte verbietet. Ohne Wahl bleibt alles beim
// System, das regelt allein das CSS. Der Umschalter steht in src/scripts/nav.ts.
(function () {
  var root = document.documentElement;
  var key = 'humanitl-theme';
  var bars = { dark: '#030405', light: '#f3f4f7' };

  // theme ist 'light', 'dark' oder null für „wie das System".
  function apply(theme) {
    if (theme) root.setAttribute('data-theme', theme);
    else root.removeAttribute('data-theme');
    var metas = document.querySelectorAll('meta[name="theme-color"]');
    for (var i = 0; i < metas.length; i++) {
      metas[i].content = bars[theme || metas[i].getAttribute('data-scheme')];
    }
    var scheme = document.querySelector('meta[name="color-scheme"]');
    if (scheme) scheme.content = theme || 'light dark';
  }

  window.humanitlTheme = { key: key, apply: apply };
  root.setAttribute('data-theme-js', '');
  try {
    var stored = localStorage.getItem(key);
    if (stored === 'light' || stored === 'dark') apply(stored);
  } catch (e) {
    // Gesperrter Speicher: Es bleibt beim System.
  }
})();
