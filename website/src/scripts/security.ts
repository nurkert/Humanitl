// Bewegung der Sicherheitsseite (HUM-345). Ohne Skript und bei reduzierter Bewegung steht
// jedes Diagramm im vollständigen Endzustand; dieses Modul fügt nur hinzu. Kein
// scroll-Listener, keine Dauerschleife: Beobachter setzen Attribute, CSS bewegt.

const root = document.documentElement;
const reduce = window.matchMedia('(prefers-reduced-motion: reduce)');
const observers: IntersectionObserver[] = [];
const timers: number[] = [];
let tamperTouched = false;
// Schmal (bis 64rem) steht die Figur angeheftet über den Schritten; das ändert Takt und Kamera.
const narrow = window.matchMedia('(max-width: 64rem)');

const figures = Array.from(document.querySelectorAll<HTMLElement>('[data-jfig]'));
const steps = Array.from(document.querySelectorAll<HTMLElement>('.sec-step'));

/* ---- Audit-Knopf: bedienbar mit und ohne Bewegung --------------------------------- */
const chain = document.querySelector<HTMLElement>('[data-chain-el]');
const tamper = document.querySelector<HTMLButtonElement>('[data-tamper]');
const tamperLive = document.querySelector<HTMLElement>('[data-tamper-live]');

function setTampered(on: boolean, announce: boolean) {
  if (!chain || !tamper) return;
  chain.classList.toggle('is-tampered', on);
  tamper.setAttribute('aria-pressed', String(on));
  if (announce && tamperLive) tamperLive.textContent = (on ? tamper.dataset.on : tamper.dataset.off) ?? '';
}

if (chain && tamper) {
  tamper.hidden = false;
  tamper.addEventListener('click', () => {
    // Eine Wahl des Nutzers beendet die automatische Vorführung.
    tamperTouched = true;
    timers.splice(0).forEach((t) => window.clearTimeout(t));
    setTampered(!chain.classList.contains('is-tampered'), true);
  });
}

/* ---- Kapitelleiste: kein Bewegungsthema, läuft immer ------------------------------- */
const rail = document.querySelector<HTMLElement>('[data-rail]');
if (rail && 'IntersectionObserver' in window) {
  const links = new Map<string, HTMLAnchorElement>();
  rail.querySelectorAll<HTMLAnchorElement>('[data-rail-link]').forEach((a) => links.set(a.dataset.railLink!, a));
  const railIO = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const link = links.get(entry.target.id);
        if (!link) continue;
        links.forEach((a) => a.removeAttribute('aria-current'));
        link.setAttribute('aria-current', 'true');
        rail.style.setProperty('--k', link.style.getPropertyValue('--k') || '0');
        rail.dataset.active = '';
      }
    },
    { rootMargin: '-45% 0px -50% 0px' },
  );
  document.querySelectorAll<HTMLElement>('section[data-chapter]').forEach((sec) => railIO.observe(sec));
}

/* ---- Bewegung --------------------------------------------------------------------- */
function setBeat(beat: string) {
  for (const fig of figures) fig.dataset.beat = beat;
}

function settle() {
  // Auf reduzierte Bewegung umschalten: alles final, Beobachter weg.
  root.removeAttribute('data-sec-live');
  observers.splice(0).forEach((o) => o.disconnect());
  timers.splice(0).forEach((t) => window.clearTimeout(t));
  setBeat('all');
  figures.forEach((fig) => fig.setAttribute('tabindex', '0'));
  document.querySelectorAll<HTMLElement>('[data-reveal]').forEach((el) => el.setAttribute('data-shown', ''));
  steps.forEach((st) => st.removeAttribute('aria-current'));
}

function start() {
  // Ein Lesedurchgang, dann ein Schreibdurchgang: kein Lesen nach Schreiben.
  const reveals = Array.from(document.querySelectorAll<HTMLElement>('[data-reveal]'));
  const vh = window.innerHeight;
  const below = reveals.filter((el) => el.getBoundingClientRect().top > vh);
  below.forEach((el) => el.setAttribute('data-pending', ''));
  root.setAttribute('data-sec-live', '');
  // Nur das statische Bild ist schmal waagrecht scrollbar und braucht dafür einen Tabstopp.
  figures.forEach((fig) => fig.removeAttribute('tabindex'));

  const revealIO = new IntersectionObserver(
    (entries, io) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const el = entry.target as HTMLElement;
        el.setAttribute('data-shown', '');
        io.unobserve(el);
        if (el.hasAttribute('data-chain') && chain && !tamperTouched) {
          timers.push(window.setTimeout(() => setTampered(true, false), 1200));
          timers.push(window.setTimeout(() => setTampered(false, false), 3600));
        }
      }
    },
    { rootMargin: '0px 0px -10% 0px', threshold: 0.2 },
  );
  below.forEach((el) => revealIO.observe(el));
  observers.push(revealIO);

  // Beats der Journey-Figur: der Schritt in der Mitte des Fensters gibt den Takt.
  if (figures.length && steps.length) {
    setBeat('1');
    let beatIO: IntersectionObserver | null = null;
    const buildBeatIO = () => {
      if (beatIO) {
        beatIO.disconnect();
        const at = observers.indexOf(beatIO);
        if (at >= 0) observers.splice(at, 1);
      }
      beatIO = new IntersectionObserver(
        (entries) => {
          for (const entry of entries) {
            if (!entry.isIntersecting) continue;
            const step = entry.target as HTMLElement;
            steps.forEach((st) => (st === step ? st.setAttribute('aria-current', 'step') : st.removeAttribute('aria-current')));
            setBeat(step.dataset.step ?? '1');
          }
        },
        // Schmal liegt die angeheftete Figur oben; der Takt hört deshalb tiefer im Fenster hin.
        { rootMargin: narrow.matches ? '-62% 0px -30% 0px' : '-45% 0px -45% 0px', threshold: 0 },
      );
      steps.forEach((st) => beatIO!.observe(st));
      observers.push(beatIO);
    };
    buildBeatIO();
    // Drehen oder Fenstergröße über die Grenze: Beobachter mit passendem Fenster neu anlegen.
    narrow.addEventListener('change', () => {
      if (root.hasAttribute('data-sec-live')) buildBeatIO();
    });

    const journey = document.querySelector<HTMLElement>('[data-journey]');
    if (journey) {
      const leaveIO = new IntersectionObserver((entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) continue;
          setBeat(entry.boundingClientRect.top < 0 ? '7' : '1');
        }
      });
      leaveIO.observe(journey);
      observers.push(leaveIO);
    }
  }

  // Schleifen laufen nur, solange ihr Diagramm sichtbar ist.
  const loopIO = new IntersectionObserver((entries) => {
    for (const entry of entries) entry.target.toggleAttribute('data-inview', entry.isIntersecting);
  });
  document.querySelectorAll<HTMLElement>('[data-loop]').forEach((el) => loopIO.observe(el));
  observers.push(loopIO);
}

document.addEventListener('visibilitychange', () => {
  root.toggleAttribute('data-paused', document.visibilityState === 'hidden');
});

if (!reduce.matches && 'IntersectionObserver' in window) {
  start();
  reduce.addEventListener('change', (event) => {
    if (event.matches) settle();
  });
}
