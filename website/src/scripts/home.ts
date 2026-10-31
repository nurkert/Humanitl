// Bewegung der Startseiten-Sektionen (HUM-358). Ohne Skript und bei reduzierter Bewegung steht
// alles im Endzustand; dieses Modul fügt nur hinzu. Kein scroll-Listener: Beobachter setzen
// Attribute, CSS bewegt. stage.ts bleibt davon unberührt.

const root = document.documentElement;
const reduce = window.matchMedia('(prefers-reduced-motion: reduce)');
const observers: IntersectionObserver[] = [];
const timers: number[] = [];
const flow = document.querySelector<HTMLElement>('[data-hm-flow]');
const replay = document.querySelector<HTMLButtonElement>('[data-hm-replay]');
// So lange dauert der Ablauf des Schemas; danach fallen seine Animationen weg, damit ein
// Wechsel zwischen querer und hoher Fassung ihn nicht neu startet.
const flowMs = 10000;
let flowTimer = 0;

function playFlow() {
  if (!flow) return;
  window.clearTimeout(flowTimer);
  flow.setAttribute('data-hm-play', '');
  flowTimer = window.setTimeout(() => flow.removeAttribute('data-hm-pending'), flowMs);
  timers.push(flowTimer);
}

// Tastatur: Fokus auf einem Element, das noch nicht aufgetreten ist, zeigt es sofort.
function onFocusIn(event: FocusEvent) {
  const target = event.target;
  if (!(target instanceof Element)) return;
  target.closest('[data-hm-pending]:not([data-hm-flow])')?.setAttribute('data-hm-shown', '');
  if (target.closest('.hm-flowfig') && flow && !flow.hasAttribute('data-hm-play')) playFlow();
}

function settle() {
  document.removeEventListener('focusin', onFocusIn);
  root.removeAttribute('data-hm-live');
  observers.splice(0).forEach((o) => o.disconnect());
  timers.splice(0).forEach((t) => window.clearTimeout(t));
  document.querySelectorAll('[data-hm-reveal]').forEach((el) => el.setAttribute('data-hm-shown', ''));
  if (flow) flow.setAttribute('data-hm-play', '');
  if (replay) replay.hidden = true;
}

function start() {
  const vh = window.innerHeight;
  const reveals = Array.from(document.querySelectorAll<HTMLElement>('[data-hm-reveal]'));
  const below = reveals.filter((el) => el.getBoundingClientRect().top > vh);
  below.forEach((el) => el.setAttribute('data-hm-pending', ''));
  root.setAttribute('data-hm-live', '');
  document.addEventListener('focusin', onFocusIn);

  // Die Zeilen des Angriffs enden nacheinander, auch wenn mehrere zugleich ins Bild kommen.
  let nextStep = 0;
  const revealIO = new IntersectionObserver(
    (entries, io) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue;
        const el = entry.target as HTMLElement;
        io.unobserve(el);
        if (!el.hasAttribute('data-hm-step')) {
          el.setAttribute('data-hm-shown', '');
          continue;
        }
        const now = performance.now();
        const at = Math.max(now, nextStep);
        nextStep = at + 750;
        timers.push(window.setTimeout(() => el.setAttribute('data-hm-shown', ''), at - now));
      }
    },
    { rootMargin: '0px 0px -12% 0px', threshold: 0.01 },
  );
  below.forEach((el) => revealIO.observe(el));
  observers.push(revealIO);

  if (flow) {
    const fig = flow.querySelector<HTMLElement>('.hm-flowfig');
    if (fig && fig.getBoundingClientRect().top > vh) {
      flow.setAttribute('data-hm-pending', '');
      const flowIO = new IntersectionObserver(
        (entries, io) => {
          if (!entries.some((entry) => entry.isIntersecting)) return;
          io.disconnect();
          playFlow();
        },
        // Ein Band in der Fenstermitte statt eines Flächenanteils: Auch auf niedrigen Fenstern,
        // in die das Schema nie zu 40 Prozent passt, startet es so.
        { threshold: 0, rootMargin: '-30% 0px -30% 0px' },
      );
      flowIO.observe(fig);
      observers.push(flowIO);
    }
    if (replay) {
      replay.hidden = false;
      replay.addEventListener('click', () => {
        // Das Attribut weg, ein Layout-Durchgang, wieder setzen: So beginnen die Animationen neu.
        flow.setAttribute('data-hm-pending', '');
        flow.removeAttribute('data-hm-play');
        void flow.getBoundingClientRect();
        playFlow();
      });
    }
  }
}

if (!reduce.matches && 'IntersectionObserver' in window) {
  start();
  reduce.addEventListener('change', (event) => {
    if (event.matches) settle();
  });
}
