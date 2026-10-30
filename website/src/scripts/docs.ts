// Komfort für die Docs-Seiten (HUM-336), gebündelt statt inline wegen der CSP:
// Kopierknöpfe an Codeblöcken, Scroll-Spy für das Inhaltsverzeichnis, Popover schließen
// nach einem Sprung und die Navigation so scrollen, dass die aktuelle Seite sichtbar ist.
// Ohne dieses Skript bleibt alles bedienbar; es fehlen nur Kopierknopf und Markierung.

const live = document.querySelector<HTMLElement>('[data-live]');
// Die Wörter des Kopierknopfs in der Sprache der Seite (HUM-346). Zeigt die Seite den Text der
// anderen Sprache, nennt <html lang> diese; die Bedienelemente tragen dann die Sprache der Seite.
const chromeLang = document.querySelector<HTMLElement>('[data-docs]')?.lang || document.documentElement.lang;
const words =
  chromeLang === 'de'
    ? {
        copy: 'Kopieren',
        copyCode: 'Code kopieren',
        copied: 'Kopiert',
        announceCopied: 'In die Zwischenablage kopiert',
        press: (keys: string) => `${keys} drücken`,
        failed: (hint: string) => `Kopieren fehlgeschlagen. Der Code ist markiert. ${hint}.`,
      }
    : {
        copy: 'Copy',
        copyCode: 'Copy code',
        copied: 'Copied',
        announceCopied: 'Copied to clipboard',
        press: (keys: string) => `Press ${keys}`,
        failed: (hint: string) => `Copying failed. The code is selected. ${hint}.`,
      };
const isMac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

function announce(message: string) {
  if (!live) return;
  live.textContent = '';
  // Neu setzen, damit Screenreader dieselbe Meldung auch ein zweites Mal vorlesen.
  window.setTimeout(() => (live.textContent = message), 30);
}

/** Ein Element innerhalb seines Scroll-Behälters sichtbar machen, ohne das Fenster zu bewegen. */
function revealInside(container: HTMLElement | null, target: HTMLElement | null, center = true) {
  if (!container || !target || container.scrollHeight <= container.clientHeight) return;
  const box = container.getBoundingClientRect();
  const rect = target.getBoundingClientRect();
  const top = rect.top - box.top + container.scrollTop;
  if (center) {
    container.scrollTop = top - container.clientHeight / 2 + rect.height / 2;
  } else if (rect.top < box.top + 16 || rect.bottom > box.bottom - 24) {
    container.scrollTop = top - container.clientHeight / 3;
  }
}

// Kopierknöpfe -------------------------------------------------------------------------

function selectText(node: Node) {
  const range = document.createRange();
  range.selectNodeContents(node);
  const selection = window.getSelection();
  selection?.removeAllRanges();
  selection?.addRange(range);
}

for (const figure of document.querySelectorAll<HTMLElement>('.doc figure.code')) {
  const pre = figure.querySelector('pre');
  if (!pre) continue;
  const button = document.createElement('button');
  button.type = 'button';
  button.className = 'code__copy';
  // Steht im Artikel, spricht aber die Sprache der Seite.
  button.lang = chromeLang;
  button.textContent = words.copy;
  button.setAttribute('aria-label', words.copyCode);
  const bar = figure.querySelector('.code__bar');
  (bar ?? figure).append(button);
  let timer = 0;
  const flash = (label: string) => {
    button.textContent = label;
    window.clearTimeout(timer);
    timer = window.setTimeout(() => (button.textContent = words.copy), 1600);
  };
  button.addEventListener('click', async () => {
    const code = pre.textContent ?? '';
    try {
      await navigator.clipboard.writeText(code);
      flash(words.copied);
      announce(words.announceCopied);
    } catch {
      selectText(pre);
      const hint = words.press(isMac ? '⌘C' : 'Ctrl+C');
      flash(hint);
      announce(words.failed(hint));
    }
  });
}

// Inhaltsverzeichnis an die Stelle unter den Metazeilen (64 bis 80rem Breite) ----------

const inlineToc = document.querySelector('[data-toc-inline]');
const meta = document.querySelector('.doc .doc-meta');
if (inlineToc && meta) meta.after(inlineToc);

// Scroll-Spy ---------------------------------------------------------------------------

const tocLinks = [...document.querySelectorAll<HTMLAnchorElement>('[data-toc] a[data-slug]')];
const slugs = [...new Set(tocLinks.map((link) => link.dataset.slug!))];
const headings = slugs
  .map((slug) => document.getElementById(slug))
  .filter((heading): heading is HTMLElement => heading !== null);
const tocScroller = document.querySelector<HTMLElement>('[data-tocnav]');
let active = '';

function setActive(slug: string) {
  if (slug === active) return;
  active = slug;
  for (const link of tocLinks) {
    if (link.dataset.slug === slug) link.setAttribute('aria-current', 'true');
    else link.removeAttribute('aria-current');
  }
  revealInside(tocScroller, tocScroller?.querySelector<HTMLElement>(`a[data-slug="${CSS.escape(slug)}"]`) ?? null, false);
}

if (headings.length) {
  const offset = 96;
  // Aktiv ist die letzte Überschrift oberhalb der Linie knapp unter der Kopfleiste. Der
  // Observer meldet nur Übergänge; die eigentliche Wahl rechnet `pick` aus den Positionen.
  const pick = () => {
    let current = headings[0].id;
    for (const heading of headings) {
      if (heading.getBoundingClientRect().top - offset <= 1) current = heading.id;
      else break;
    }
    setActive(current);
  };
  const observer = new IntersectionObserver(pick, { rootMargin: `-${offset}px 0px -66% 0px` });
  for (const heading of headings) observer.observe(heading);
  // Der Observer allein verpasst Sprünge, bei denen eine Überschrift in einem Schritt von
  // unterhalb des Fensters über die Linie springt (Pos1/Ende, Scrollbalken, Suche, Fokus in
  // eine lange Tabelle). Darum zusätzlich beim Scrollen prüfen, höchstens einmal pro Frame.
  let queued = false;
  const schedule = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      pick();
    });
  };
  window.addEventListener('scroll', schedule, { passive: true });
  window.addEventListener('resize', schedule, { passive: true });
  window.addEventListener('hashchange', pick);
  pick();
}

// Popover auf dem Telefon --------------------------------------------------------------

for (const sheet of document.querySelectorAll<HTMLElement>('.sheet[popover]')) {
  sheet.addEventListener('click', (event) => {
    const link = (event.target as Element).closest('a[href^="#"]');
    if (link) sheet.hidePopover();
  });
  sheet.addEventListener('toggle', (event) => {
    if ((event as ToggleEvent).newState !== 'open') return;
    const current = sheet.querySelector<HTMLElement>('[aria-current]');
    revealInside(sheet, current);
  });
}

// Seitenleiste: die aktuelle Seite sichtbar, nur innerhalb der Leiste gescrollt.
const sidenav = document.querySelector<HTMLElement>('[data-sidenav]');
revealInside(sidenav, sidenav?.querySelector<HTMLElement>('[aria-current="page"]') ?? null);

// Sprachwechsel: dieselbe Stelle in der anderen Sprache (HUM-346). Die Überschriften einer
// Übersetzung tragen die IDs des Originals, darum reicht es, den aktuellen Anker mitzunehmen.
// Ohne Skript führt der Link auf den Anfang derselben Seite.
document.addEventListener('click', (event) => {
  const link = (event.target as Element | null)?.closest<HTMLAnchorElement>('.nav__lang a[hreflang], a[data-lang-link]');
  if (!link || !location.hash || link.hash) return;
  link.hash = location.hash;
});
