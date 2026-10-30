// Die Bühne der Startseite. Ein angehefteter Abschnitt, drei Phasen über dem Scrollweg:
//
//   1. Halten:  der Fächer unter der Überschrift, wie nach dem Einflug.
//   2. Morph:   die Fenster zoomen nach vorn und reihen sich auf dem gebogenen Ring auf.
//   3. Module:  jedes Stück Scrollweg dreht den Ring um ein Fenster weiter; nach dem
//               letzten löst sich die Bühne, und die Seite scrollt normal weiter.
//
// Der Ring ist rund: Pfeile, Wischen und die Pfeiltasten drehen ihn ohne Ende weiter
// (Round Robin), unabhängig vom Scrollweg. Jede Bewegung ist ein transform pro Bild,
// gerechnet nur, wenn sich etwas geändert hat.

const scene = document.querySelector<HTMLElement>('[data-scene]');
if (scene) init(scene);

interface Pose {
  x: number;
  y: number;
  z: number;
  ry: number;
  rx: number;
  rz: number;
  s: number;
  o: number;
  lit: number;
}

function init(scene: HTMLElement) {
  const wins = [...scene.querySelectorAll<HTMLElement>('[data-win]')];
  const hero = scene.querySelector<HTMLElement>('[data-hero]')!;
  const actions = scene.querySelector<HTMLElement>('[data-hero-actions]')!;
  const deck = scene.querySelector<HTMLElement>('[data-deck]')!;
  const ring = scene.querySelector<HTMLElement>('[data-ring]')!;
  const world = scene.querySelector<HTMLElement>('[data-world]')!;
  const fill = scene.querySelector<HTMLElement>('[data-fill]')!;
  const captionName = scene.querySelector<HTMLElement>('[data-caption-name]')!;
  const captionTitle = scene.querySelector<HTMLElement>('[data-caption-title]')!;
  const captionText = scene.querySelector<HTMLElement>('[data-caption-text]')!;
  const n = wins.length;
  const reduced = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // Scrollweg je Phase, in Bildschirmhöhen.
  const HOLD = 0.35;
  const MORPH = 0.9;
  const STEP = 0.55;
  const TAIL = 0.35;

  let vw = 0;
  let vh = 0;
  let width = 0;
  let sceneTop = 0;
  let narrow = false;
  // Der Fächer steht zwischen Überschrift und Knöpfen: seine Mitte und seine Größe.
  let fanY = 0;
  let fanScale = 0.7;

  // Zustand: Morph 0..1, Ringposition aus dem Scrollweg, dazu die Drehung von Hand.
  let morph = 0;
  let scrollPos = 0;
  let manual = 0;
  let manualFrom = 0;
  let manualTo = 0;
  let manualStart = 0;
  let introStart = -1;
  let active = -1;
  let frame = 0;

  const ASPECT = 2560 / 1380;
  const INTRO_MS = 1500;
  const STAGGER_MS = 110;
  const TURN_MS = 750;

  const clamp = (v: number, a = 0, b = 1) => Math.min(b, Math.max(a, v));
  const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
  const expoOut = (t: number) => (t >= 1 ? 1 : 1 - Math.pow(2, -10 * t));
  const smooth = (t: number) => t * t * (3 - 2 * t);
  // Abstand auf dem Ring, gefaltet in [-n/2, n/2): so hat jedes Fenster links und rechts Nachbarn.
  const wrap = (d: number) => ((((d + n / 2) % n) + n) % n) - n / 2;
  const mod = (v: number) => ((v % n) + n) % n;

  function measure() {
    vw = window.innerWidth;
    vh = window.innerHeight;
    narrow = vw < 832;
    width = narrow ? vw * 0.92 : Math.min(vw * 0.62, 1120);
    const total = HOLD + MORPH + (n - 1) * STEP + TAIL + 1;
    scene.style.setProperty('--scene-h', `${Math.round(total * vh)}px`);
    sceneTop = scene.getBoundingClientRect().top + window.scrollY;
    const top = hero.offsetTop + hero.offsetHeight;
    const bottom = actions.offsetTop;
    const room = Math.max(bottom - top, vh * 0.2);
    fanY = (top + bottom) / 2 - vh / 2;
    fanScale = Math.min(narrow ? 0.95 : 0.7, (room * 0.86) / (width / ASPECT));
    ring.style.width = `${(RADIUS() * 2.08).toFixed(0)}px`;
  }

  // Der Fächer unter der Überschrift: fünf Fenster im leichten Bogen, nach hinten gekippt.
  function fanPose(d: number): Pose {
    const a = Math.abs(d);
    const spread = narrow ? 0.3 : 0.34;
    return {
      x: d * width * spread * (fanScale / 0.7) * (1 - a * 0.05),
      y: fanY + a * a * width * 0.012 * fanScale,
      z: -a * width * 0.32 * (fanScale / 0.7),
      ry: -d * 17,
      rx: 9,
      rz: d * 1.4,
      s: fanScale,
      o: clamp(1 - Math.max(0, a - (narrow ? 1.2 : 2.2))),
      lit: 1 - a * 0.1,
    };
  }

  // Die Bühne: Fenster auf einem Zylinder um den Betrachter, das vordere groß und scharf.
  function stagePose(d: number): Pose {
    const a = Math.abs(d);
    const phi = d * STEP_ANGLE();
    const radius = RADIUS();
    return {
      x: Math.sin(phi) * radius,
      y: -vh * 0.02,
      z: -(1 - Math.cos(phi)) * radius,
      ry: (-phi * 180) / Math.PI,
      rx: 0,
      rz: 0,
      s: STAGE_SCALE(),
      o: clamp(1 - Math.max(0, a - 2.2) * 0.8),
      lit: 1 - Math.min(a, 3) * 0.26,
    };
  }

  // Der Zylinder der Bühne: Winkel zwischen zwei Fenstern, Radius, Größe des vorderen.
  const STEP_ANGLE = () => (narrow ? 36 : 30) * (Math.PI / 180);
  const RADIUS = () => width * (narrow ? 1.2 : 1.4);
  const STAGE_SCALE = () => (narrow ? 0.9 : 0.86);

  // Der Startpunkt des Einflugs: tief unten, flach gekippt, unsichtbar.
  function introPose(base: Pose, d: number): Pose {
    return { ...base, y: base.y + vh * 0.55, z: base.z - width * 0.5, rx: 38, ry: base.ry - d * 6, s: base.s * 0.82, o: 0, lit: 0.4 };
  }

  function mix(p: Pose, q: Pose, t: number): Pose {
    return {
      x: lerp(p.x, q.x, t),
      y: lerp(p.y, q.y, t),
      z: lerp(p.z, q.z, t),
      ry: lerp(p.ry, q.ry, t),
      rx: lerp(p.rx, q.rx, t),
      rz: lerp(p.rz, q.rz, t),
      s: lerp(p.s, q.s, t),
      o: lerp(p.o, q.o, t),
      lit: lerp(p.lit, q.lit, t),
    };
  }

  function read() {
    const p = window.scrollY - sceneTop;
    const hold = HOLD * vh;
    const morphLen = MORPH * vh;
    morph = clamp((p - hold) / morphLen);
    scrollPos = clamp((p - hold - morphLen) / (STEP * vh), 0, n - 1);
  }

  function render(now: number) {
    frame = 0;
    let busy = false;

    // Drehung von Hand: weich zum Ziel, über das Ende hinaus ohne Sprung.
    if (manual !== manualTo) {
      const t = reduced ? 1 : clamp((now - manualStart) / TURN_MS);
      manual = t >= 1 ? manualTo : lerp(manualFrom, manualTo, expoOut(t));
      busy ||= t < 1;
    }
    const position = scrollPos + manual;
    const m = smooth(morph);

    let introDone = true;
    wins.forEach((win, i) => {
      const fanD = wrap(i);
      const stageD = wrap(i - position);
      let pose = mix(fanPose(fanD), stagePose(stageD), m);
      if (introStart >= 0) {
        const order = Math.abs(fanD);
        const t = clamp((now - introStart - order * STAGGER_MS) / INTRO_MS);
        if (t < 1) {
          introDone = false;
          pose = mix(introPose(pose, fanD), pose, expoOut(t));
        }
      }
      const front = 1 - clamp(Math.abs(stageD));
      win.style.transform =
        `translate3d(-50%, -50%, 0) translate3d(${pose.x.toFixed(1)}px, ${pose.y.toFixed(1)}px, ${pose.z.toFixed(1)}px) ` +
        `rotateY(${pose.ry.toFixed(2)}deg) rotateX(${pose.rx.toFixed(2)}deg) rotateZ(${pose.rz.toFixed(2)}deg) scale(${pose.s.toFixed(4)})`;
      win.style.opacity = pose.o.toFixed(3);
      // Licht und Schatten sind eigene Ebenen: Deckkraft und Verschiebung, nie ein Filter.
      win.style.setProperty('--shade', (1 - pose.lit).toFixed(3));
      win.style.setProperty('--sx', `${(-pose.ry * width * 0.012).toFixed(1)}px`);
      win.style.setProperty('--glow', (front * m).toFixed(3));
      win.setAttribute('aria-hidden', Math.abs(stageD) < 0.5 || m < 0.5 ? 'false' : 'true');
    });
    if (introDone) introStart = -1;
    busy ||= !introDone;

    // Überschrift und Knöpfe gehen nach oben weg, die Steuerung der Bühne kommt.
    const heroOut = clamp(m * 1.8);
    hero.style.opacity = (1 - heroOut).toFixed(3);
    hero.style.transform = `translate3d(0, ${(-heroOut * vh * 0.12).toFixed(1)}px, 0) scale(${(1 - heroOut * 0.04).toFixed(4)})`;
    // Die Überschrift bleibt im Baum der Zugänglichkeit; sie verschwindet nur fürs Auge.
    actions.style.opacity = (1 - clamp(m * 3)).toFixed(3);
    actions.style.transform = `translate3d(0, ${(clamp(m * 3) * 24).toFixed(1)}px, 0)`;
    actions.style.visibility = m >= 0.34 ? 'hidden' : 'visible';
    const deckIn = clamp((m - 0.55) / 0.45);
    deck.style.opacity = deckIn.toFixed(3);
    deck.toggleAttribute('data-live', deckIn > 0.6);
    // Vor dem Morph ist die Steuerung unsichtbar und darf weder Fokus noch Ansagen bekommen.
    deck.inert = deckIn <= 0.6;
    // Der Ring ist der Boden des Zylinders: Mitte im Zentrum der Bühne, die Fenster stehen
    // mit ihrer Unterkante auf seiner vorderen Kante.
    const radius = RADIUS();
    const base = -vh * 0.02 + (width / ASPECT) * STAGE_SCALE() * 0.5 + 14;
    ring.style.opacity = (m * 0.95).toFixed(3);
    ring.style.transform =
      `translate3d(-50%, -50%, 0) translate3d(0, ${(base + (1 - m) * vh * 0.3).toFixed(1)}px, ${(-radius).toFixed(1)}px) ` +
      `rotateX(90deg) scale(${lerp(0.82, 1, m).toFixed(4)})`;

    const now_active = mod(Math.round(position));
    if (now_active !== active) {
      active = now_active;
      const win = wins[active];
      captionName.textContent = win.dataset.name ?? '';
      captionTitle.textContent = win.dataset.title ?? '';
      captionText.textContent = win.dataset.text ?? '';
    }
    fill.style.setProperty('--fill', ((mod(position) + 1) / n).toFixed(4));

    if (busy) schedule();
  }

  function schedule() {
    if (!frame) frame = requestAnimationFrame(render);
  }

  function turn(by: number) {
    manualFrom = manual;
    manualTo = Math.round(manualTo) + by;
    manualStart = performance.now();
    schedule();
  }

  // Rastet nach dem Scrollen auf das nächste ganze Fenster ein, oder auf Anfang und Ende
  // des Morphs, damit nie ein halber Zustand stehen bleibt.
  function settle() {
    const p = window.scrollY - sceneTop;
    const hold = HOLD * vh;
    const morphLen = MORPH * vh;
    const stepLen = STEP * vh;
    let target: number | undefined;
    if (morph > 0 && morph < 1) {
      target = morph < 0.5 ? hold : hold + morphLen;
    } else if (morph >= 1 && scrollPos < n - 1) {
      const nearest = Math.round(scrollPos);
      if (Math.abs(scrollPos - nearest) > 0.01) target = hold + morphLen + nearest * stepLen;
    }
    if (target === undefined || Math.abs(target - p) < 2) return;
    window.scrollTo({ top: sceneTop + target, behavior: reduced ? 'auto' : 'smooth' });
  }

  let settleTimer = 0;
  const onScroll = () => {
    read();
    schedule();
    if (!('onscrollend' in window)) {
      clearTimeout(settleTimer);
      settleTimer = window.setTimeout(settle, 160);
    }
  };
  window.addEventListener('scroll', onScroll, { passive: true });
  window.addEventListener('scrollend', settle);
  window.addEventListener('resize', () => {
    measure();
    read();
    schedule();
  });

  scene.querySelector('[data-prev]')!.addEventListener('click', () => turn(-1));
  scene.querySelector('[data-next]')!.addEventListener('click', () => turn(1));
  // Die Pfeiltasten wirken, solange die Bühne steht, egal wo der Fokus ist; nur nicht in
  // Eingabefeldern und nicht mit Zusatztasten.
  document.addEventListener('keydown', (event) => {
    if (morph < 1 || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest('input, textarea, select, [contenteditable]')) return;
    const p = window.scrollY - sceneTop;
    if (p < 0 || p > scene.offsetHeight - vh) return;
    if (event.key === 'ArrowLeft') turn(-1);
    else if (event.key === 'ArrowRight') turn(1);
    else return;
    event.preventDefault();
  });

  // Wischen und waagrechtes Scrollen drehen den Ring; senkrecht bleibt Sache der Seite.
  let dragX = 0;
  let dragging = false;
  world.addEventListener('pointerdown', (event) => {
    if (morph < 1) return;
    dragging = true;
    dragX = event.clientX;
  });
  window.addEventListener('pointercancel', () => (dragging = false));
  window.addEventListener('pointerup', (event) => {
    if (!dragging) return;
    dragging = false;
    const dx = event.clientX - dragX;
    if (Math.abs(dx) > 40) turn(dx < 0 ? 1 : -1);
  });
  let wheelLock = 0;
  world.addEventListener(
    'wheel',
    (event) => {
      if (morph < 1 || Math.abs(event.deltaX) < Math.abs(event.deltaY) * 1.2 || Math.abs(event.deltaX) < 12) return;
      event.preventDefault();
      const now = performance.now();
      if (now < wheelLock) return;
      wheelLock = now + 420;
      turn(event.deltaX > 0 ? 1 : -1);
    },
    { passive: false },
  );

  measure();
  read();
  if (!reduced) introStart = performance.now();
  schedule();
  // Schriften und Bilder können die Höhe oben noch verschieben.
  window.addEventListener('load', () => {
    measure();
    read();
    schedule();
  });
}
