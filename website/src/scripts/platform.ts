// Das System des Besuchers, soweit es für Humanitl zählt (HUM-337). Leer heißt: keines
// von beiden, dann bleibt die Seite neutral und spricht für niemanden eine Wahl aus.
export function detectPlatform(): 'macos' | 'linux' | '' {
  const data = (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData;
  const platform = (data?.platform || navigator.platform || navigator.userAgent).toLowerCase();
  // Telefone und Tablets zählen nicht: Android meldet sich als Linux, das iPad im
  // Desktop-Modus als Mac, und keines von beiden führt Humanitl aus.
  if (/android|iphone|ipad|ipod/i.test(navigator.userAgent) || (navigator.maxTouchPoints > 1 && platform.includes('mac'))) return '';
  if (platform.includes('mac')) return 'macos';
  if (platform.includes('linux') || platform.includes('x11')) return 'linux';
  return '';
}
