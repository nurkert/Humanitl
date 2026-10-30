// Die Download-Seite: wählt das System des Besuchers vor, schaltet zwischen den Feldern
// und kopiert Befehle. Ohne Skript stehen beide Felder offen untereinander.

const tabs = document.querySelector<HTMLElement>('[data-tabs]');
if (tabs) initTabs(tabs);

function detect(): string {
  const data = (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData;
  const platform = (data?.platform || navigator.platform || navigator.userAgent).toLowerCase();
  // Telefone und Tablets bekommen keine Empfehlung: Android meldet sich als Linux, das iPad
  // im Desktop-Modus als Mac, und keines von beiden führt Humanitl aus.
  if (/android|iphone|ipad|ipod/i.test(navigator.userAgent) || navigator.maxTouchPoints > 1 && platform.includes('mac')) return '';
  if (platform.includes('mac')) return 'macos';
  if (platform.includes('linux') || platform.includes('x11')) return 'linux';
  return '';
}

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
  const detected = detect();
  const recommended = buttons.find((button) => button.dataset.tab === detected);
  recommended?.querySelector<HTMLElement>('[data-recommended]')?.removeAttribute('hidden');
  // Ohne Übergang setzen, dann erst die Bewegung des Gleiters erlauben.
  glider?.style.setProperty('transition', 'none');
  select(recommended?.dataset.tab ?? buttons[0].dataset.tab!);
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
