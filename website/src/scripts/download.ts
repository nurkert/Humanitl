// Die Download-Seite: öffnet das Feld des Systems, auf dem sie gerade läuft (still, ohne
// Empfehlung auszusprechen), schaltet zwischen den Feldern
// und kopiert Befehle. Ohne Skript stehen beide Felder offen untereinander.

import { detectPlatform } from './platform';

const tabs = document.querySelector<HTMLElement>('[data-tabs]');
if (tabs) initTabs(tabs);

function initTabs(tabs: HTMLElement) {
  const buttons = [...tabs.querySelectorAll<HTMLButtonElement>('[data-tab]')];
  const panels = [...document.querySelectorAll<HTMLElement>('[data-panel]')];
  const glider = tabs.querySelector<HTMLElement>('.tabs__glider');

  function place(button: HTMLButtonElement) {
    if (!glider) return;
    tabs.style.setProperty('--gx', `${button.offsetLeft}px`);
    tabs.style.setProperty('--gw', `${button.offsetWidth}px`);
  }

  function select(id: string, focus = false) {
    buttons.forEach((button) => {
      const on = button.dataset.tab === id;
      button.setAttribute('aria-selected', on ? 'true' : 'false');
      button.tabIndex = on ? 0 : -1;
      if (on) {
        place(button);
        if (focus) button.focus();
      }
    });
    panels.forEach((panel) => (panel.hidden = panel.dataset.panel !== id));
  }

  tabs.hidden = false;
  const detected = detectPlatform();
  const current = buttons.find((button) => button.dataset.tab === detected);
  // Ohne Übergang setzen, dann erst die Bewegung des Gleiters erlauben.
  glider?.style.setProperty('transition', 'none');
  select(current?.dataset.tab ?? buttons[0].dataset.tab!);
  requestAnimationFrame(() => glider?.style.removeProperty('transition'));

  buttons.forEach((button, i) => {
    button.addEventListener('click', () => select(button.dataset.tab!));
    button.addEventListener('keydown', (event) => {
      const keys: Record<string, number> = { ArrowRight: i + 1, ArrowLeft: i - 1, Home: 0, End: buttons.length - 1 };
      if (!(event.key in keys)) return;
      event.preventDefault();
      const next = buttons[(keys[event.key] + buttons.length) % buttons.length];
      select(next.dataset.tab!, true);
    });
  });
  window.addEventListener('resize', () => {
    const selected = buttons.find((button) => button.getAttribute('aria-selected') === 'true');
    if (selected) place(selected);
  });
  document.fonts?.ready.then(() => {
    const selected = buttons.find((button) => button.getAttribute('aria-selected') === 'true');
    if (selected) place(selected);
  });
}

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
