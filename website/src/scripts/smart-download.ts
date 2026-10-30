// Der Download-Knopf der Startseite nennt das System des Besuchers (HUM-337). Auf dem Mac
// führt er direkt zum Disk-Image; unter Linux zur Download-Seite, denn dort gibt es
// mehrere Wege. Ohne Skript und auf anderen Geräten bleibt der neutrale Knopf stehen.
import { detectPlatform } from './platform';

const platform = detectPlatform();
if (platform) {
  for (const link of document.querySelectorAll<HTMLAnchorElement>('[data-smart-download]')) {
    // Ohne Disk-Image (Release fehlt, API beim Bauen nicht erreichbar) bleibt der Knopf neutral.
    if (platform === 'macos' && !link.dataset.macHref) continue;
    if (platform === 'macos') link.href = link.dataset.macHref!;
    const label = link.querySelector<HTMLElement>('[data-smart-label]');
    const text = platform === 'macos' ? link.dataset.labelMacos : link.dataset.labelLinux;
    if (label && text) label.textContent = text;
  }
}
