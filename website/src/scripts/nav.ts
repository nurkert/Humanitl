// Die Navigation bekommt ihren Glaskörper, sobald die Seite sich bewegt hat.
const nav = document.querySelector<HTMLElement>('[data-nav]');
if (nav) {
  const update = () => nav.toggleAttribute('data-scrolled', window.scrollY > 24);
  update();
  window.addEventListener('scroll', update, { passive: true });
}

// Der Umschalter (HUM-338): wechselt zwischen Hell und Dunkel. Wer wieder beim Stand des Systems
// ankommt, löst die Wahl, dann folgt die Seite dem System erneut, auch live. Das Setzen selbst
// liegt in public/theme.js, damit es vor dem ersten Bild und hier dasselbe ist.
type Theme = 'light' | 'dark';
declare global {
  interface Window {
    humanitlTheme?: { key: string; apply: (theme: Theme | null) => void };
  }
}
const toggle = document.querySelector<HTMLButtonElement>('[data-theme-toggle]');
const themes = window.humanitlTheme;
if (toggle && themes) {
  const root = document.documentElement;
  const light = window.matchMedia('(prefers-color-scheme: light)');
  const system = (): Theme => (light.matches ? 'light' : 'dark');
  const next = (): Theme => ((root.dataset.theme ?? system()) === 'dark' ? 'light' : 'dark');
  const label = () => {
    const text = next() === 'light' ? toggle.dataset.labelLight : toggle.dataset.labelDark;
    toggle.setAttribute('aria-label', text ?? '');
    toggle.title = text ?? '';
  };
  const show = (theme: Theme | null) => {
    root.setAttribute('data-theme-flip', '');
    themes.apply(theme);
    label();
    requestAnimationFrame(() => requestAnimationFrame(() => root.removeAttribute('data-theme-flip')));
  };
  toggle.addEventListener('click', () => {
    const theme = next();
    const choice = theme === system() ? null : theme;
    try {
      if (choice) localStorage.setItem(themes.key, choice);
      else localStorage.removeItem(themes.key);
    } catch {
      // Gesperrter Speicher: Die Wahl gilt nur für diese Seite.
    }
    show(choice);
  });
  light.addEventListener('change', label);
  // Andere Tabs derselben Seite ziehen nach.
  window.addEventListener('storage', (event) => {
    if (event.key === themes.key) show(event.newValue === 'light' || event.newValue === 'dark' ? event.newValue : null);
  });
  label();
}

export {};
