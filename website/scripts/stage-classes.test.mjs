// Tests der Klassen und der Zusammenstellung der Bühne (src/scripts/stage/classes.mjs,
// assemble.mjs, HUM-413): Grenzen der Klassen, stimmige Vorgaben je Klasse, nichts Sichtbares
// über das Ringende, und das Fenster mit dem Ablauf gibt es nur im Stapel.
//
//   node --test scripts/
import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';

import { assemble } from '../src/scripts/stage/assemble.mjs';
import { HERO_NEG, REWIND, STAGE_NEG, opacityOf } from '../src/scripts/stage/choreography.mjs';
import {
  BELOW,
  BOUNDS,
  CTL_UNDER,
  EDGE,
  FLAT,
  H_BOUNDS,
  NOJS_MEDIA,
  SIDE,
  SPEC,
  classify,
  heightMarks,
  nextClass,
} from '../src/scripts/stage/classes.mjs';
import { headClassScript } from '../src/scripts/stage/head-class.mjs';
import { heightOf, params, perspectiveOf, quadWith } from '../src/scripts/stage/geometry.mjs';

test('classify ordnet die üblichen Bildausschnitte ihren Klassen zu', () => {
  const cases = [
    [2560, 1080, 'A'],
    [1770, 870, 'A'],
    [1920, 1080, 'B'],
    [1366, 768, 'B'],
    [1440, 900, 'C'],
    [1280, 800, 'C'],
    [1024, 768, 'D'],
    [768, 1024, 'E'],
    [390, 844, 'F'],
    [320, 640, 'F'],
    [844, 390, 'G'],
  ];
  for (const [w, h, cls] of cases) assert.equal(classify(w, h), cls, `${w}x${h}`);
});

test('jede Klasse hat stimmige Vorgaben', () => {
  for (const [cls, spec] of Object.entries(SPEC)) {
    const stage = params({ ...FLAT, ...spec.stage });
    assert.ok(spec.phisMin <= stage.phis, `${cls}: phisMin über phis`);
    assert.ok(spec.betaMax <= stage.beta, `${cls}: betaMax flacher als beta`);
    assert.ok(spec.ws(1440, 900, 1.89) > 0 && spec.morph > 0, cls);
    assert.ok(cls in EDGE, cls);
    // Eine Klasse steht entweder seitlich oder unter der Bühne beschriftet, nie beides.
    assert.ok(!(SIDE.has(cls) && BELOW.has(cls)), cls);
    // Die Steuerung unter der Front gibt es nur dort, wo die Beschriftung nicht darunter steht.
    if (CTL_UNDER.has(cls)) assert.ok(!BELOW.has(cls), cls);
  }
});

test('über das Ringende ist in keiner Klasse etwas zu sehen, auch nicht im Fächer von A', () => {
  // A zeigt im Fächer vier Scheiben (d = 3 halb); erst ab 3,4 ist alles aus, vor dem Sprung bei 3,5.
  for (const [cls, spec] of Object.entries(SPEC)) {
    const c = { heroProfile: spec.heroProfile, heroNeg: HERO_NEG, stagePos: spec.stagePos, stageNeg: STAGE_NEG };
    for (let m = 0; m <= 1; m += 0.05)
      for (const d of [-3.5, -3.45, -3.4, 3.4, 3.45, 3.49]) assert.equal(opacityOf(c, d, m), 0, `${cls} d=${d} m=${m}`);
  }
});

const Ws = 800;
const VIEW = { vw: 1440, vh: 900, Ws, H: heightOf(Ws, 1.8935), P: perspectiveOf(Ws) };
const C = {
  hero: params({ ...SPEC.C.hero, S: 0.8, cx: 280 }),
  stage: params({ ...FLAT, ...SPEC.C.stage, cx: -260 }),
  Ws,
  H: VIEW.H,
  stack: false,
  heroProfile: SPEC.C.heroProfile,
  heroNeg: HERO_NEG,
  stagePos: SPEC.C.stagePos,
  stageNeg: STAGE_NEG,
};

test('das Fenster mit dem Ablauf steht oben vorn und ist ab REWIND links aus dem Bild verschwunden', () => {
  const ring = { n: 7, pos: 0, target: 0, steps: 0, lead: true, reduced: false };
  const top = assemble(C, C.hero, VIEW, { ...ring, m: 0 });
  assert.equal(top.ds.length, 8);
  assert.equal(top.ds[7], -0);
  assert.equal(top.ops[7], 1);
  // Modul 1 steht direkt dahinter, das Modul auf Platz 0 ist verborgen.
  assert.ok(Math.abs(top.ds[0] - 1) < 1e-9 && top.ops[0] > 0.9);
  assert.equal(top.ops[top.ds.findIndex((d, i) => i < 7 && Math.abs(d) < 1e-9)], 0);
  for (const m of [0.3, 0.45, 0.49]) {
    const mid = assemble(C, C.hero, VIEW, { ...ring, m });
    const right = Math.max(...quadWith(VIEW, C.hero, mid.poses[7]).map((q) => q[0]));
    if (m >= 0.45) assert.ok(right < 0, `m=${m}: rechte Kante ${right.toFixed(0)}`);
  }
  const stage = assemble(C, C.stage, VIEW, { ...ring, m: REWIND[0] });
  assert.equal(stage.ops[7], 0);
  assert.equal(stage.exiting, false);
});

test('ohne Fenster mit dem Ablauf gibt es genau sieben Einträge', () => {
  const r = assemble(C, C.stage, VIEW, { n: 7, pos: 2, target: 2, steps: 0, m: 1, lead: false, reduced: false });
  assert.equal(r.ds.length, 7);
  assert.equal(r.ds[2], 0);
});

// ---- Klasse vor dem ersten Bild (HUM-418) ----------------------------------------------
// /stage-class.js setzt die Klasse an <html>, bevor der Körper gelesen ist; das CSS hängt daran,
// die Bühne übernimmt sie. Diese Tests halten Kopfskript, classify und die Fassung ohne Skript
// gleich: weicht eines ab, stünde der Text vor der Bühne anders als danach.

/** Maße für die Vergleiche: ein grobes Raster und jede Grenze genau und knapp daneben. */
function sizes() {
  const out = [];
  for (let w = 280; w <= 2600; w += 37) for (let h = 280; h <= 1600; h += 29) out.push([w, h]);
  for (const [p, q] of [BOUNDS.wide, BOUNDS.side, BOUNDS.c, BOUNDS.d])
    for (const h of [360, 480, 499, 500, 501, 800, 1000, 1080])
      for (const dw of [-1, 0, 1]) out.push([Math.round((h * p) / q) + dw, h]);
  for (const h of [BOUNDS.lowH - 1, BOUNDS.lowH, BOUNDS.lowH + 1, ...H_BOUNDS.flatMap((b) => [b, b + 1])])
    for (const w of [400, 700, 801, 820, 900, 1000, 1200]) out.push([w, h]);
  for (const w of [BOUNDS.phoneW - 1, BOUNDS.phoneW, BOUNDS.phoneW + 1])
    for (const h of [500, 600, 610, 620, 700, 900, 1200]) out.push([w, h]);
  return out;
}

/** Führt das Kopfskript mit den Maßen w x h aus; liefert die Attribute an <html>. */
function runHead(w, h) {
  const attrs = {};
  const document = { documentElement: { setAttribute: (k, v) => (attrs[k] = v) } };
  runInNewContext(headClassScript(), { window: { innerWidth: w, innerHeight: h }, document });
  return attrs;
}

test('das Kopfskript teilt jedes Maß ein wie classify und setzt die Höhenmarken', () => {
  for (const [w, h] of sizes()) {
    const attrs = runHead(w, h);
    assert.equal(attrs['data-cls'], classify(w, h), `${w}x${h}`);
    assert.equal(attrs['data-cls-w'], String(w), `${w}x${h}`);
    const marks = Object.keys(attrs).filter((k) => k.startsWith('data-h')).map((k) => Number(k.slice(6)));
    assert.deepEqual(marks, heightMarks(h), `${w}x${h}`);
  }
});

test('die Bühne teilt nur bei anderer Breite neu ein, auch nach Drehen vor dem Start', () => {
  // Gedreht zwischen Kopfskript (390 breit, F) und Bühne (844x390): neu, also G.
  assert.equal(nextClass('F', 390, 844, 390), 'G');
  // Nur die Höhe ändert sich (Adressleiste): die Klasse bleibt, auch wo classify anders wäre.
  assert.equal(nextClass('C', 1024, 1024, 768), 'C');
  assert.equal(classify(1024, 768), 'D');
  // Ohne gültige Vorgabe oder Breite: classify.
  assert.equal(nextClass('', 1024, 1024, 768), 'D');
  assert.equal(nextClass('C', NaN, 1024, 768), 'D');
});

/** Wertet eine Media Query in der Form von NOJS_MEDIA (min-/max-) für ganzzahlige Maße aus. */
function matchesOld(query, w, h) {
  return query.split(/\s+and\s+/).every((cond) => {
    const m = cond.match(/^\((?:(min|max)-)?(aspect-ratio|width|height): (\d+)(?:\/(\d+)|px)\)$/);
    assert.ok(m, `unbekannte Bedingung ${cond}`);
    const [, dir, feat, a, b] = m;
    const [l, r] = feat === 'aspect-ratio' ? [w * Number(b), Number(a) * h] : [feat === 'width' ? w : h, Number(a)];
    return dir === 'min' ? l >= r : dir === 'max' ? l <= r : l === r;
  });
}

test('die Fassung ohne Skript bildet die seitlichen Klassen nach', () => {
  for (const [w, h] of sizes()) {
    const cls = classify(w, h);
    assert.equal(matchesOld(NOJS_MEDIA.side, w, h), SIDE.has(cls), `side ${w}x${h}`);
    assert.equal(matchesOld(NOJS_MEDIA.wide, w, h), cls === 'A', `wide ${w}x${h}`);
    assert.equal(matchesOld(NOJS_MEDIA.phone, w, h), w < BOUNDS.phoneW, `phone ${w}x${h}`);
    // low nimmt genau 8/5 mit, exact setzt es dort auf C zurück; zusammen genau G.
    assert.equal(matchesOld(NOJS_MEDIA.low, w, h) && !matchesOld(NOJS_MEDIA.exact, w, h), cls === 'G', `low ${w}x${h}`);
  }
});

/** Die Vorlaufzeilen aller @media in einem Text. */
const mediaOf = (text) => [...text.matchAll(/@media ([^{]+)\{/g)].map((m) => m[1].trim());

test('Home.astro hängt die Klassen an html[data-cls], ohne Skript an NOJS_MEDIA', () => {
  const src = readFileSync(new URL('../src/components/pages/Home.astro', import.meta.url), 'utf8');
  const noscript = src.slice(src.indexOf('<noscript>'), src.indexOf('</noscript>'));
  const scoped = src.slice(src.indexOf('<style>', src.indexOf('</script>')));
  // Mit Skript: keine Query über Seitenverhältnis oder Höhe, die der Adressleiste folgte.
  for (const q of mediaOf(scoped)) assert.doesNotMatch(q, /aspect-ratio|height/, `@media ${q}`);
  assert.doesNotMatch(scoped, /\.scene\[data-cls/);
  // Jede Klasse steht in :global(:where(html[data-cls=…])); Höhenmarken nur aus H_BOUNDS.
  const wheres = [...scoped.matchAll(/:global\(:where\(([^)]*)\)\)/g)].map((m) => m[1]);
  assert.ok(wheres.length > 30, `nur ${wheres.length} Klassenregeln`);
  for (const w of wheres)
    for (const part of w.split(/\s*,\s*/))
      assert.match(part, /^html\[data-cls='[A-G]'\](\[data-h(\d+)\])?$/, part);
  for (const [, b] of scoped.matchAll(/\[data-h(\d+)\]/g)) assert.ok(H_BOUNDS.includes(Number(b)), `data-h${b}`);
  assert.equal(scoped.split("data-cls='").length - 1, [...scoped.matchAll(/html\[data-cls='/g)].length);
  // Ohne Skript: nur die alte Form aus NOJS_MEDIA, keine Bereichssyntax.
  const allowed = new Set(Object.values(NOJS_MEDIA));
  const queries = mediaOf(noscript);
  assert.ok(queries.length >= 4);
  for (const q of queries) assert.ok(allowed.has(q), `@media ${q} in noscript`);
});

test('die gebaute Seite trägt Kopfskript und Queries unverändert (nur nach npm run build)', (t) => {
  const dist = new URL('../dist/', import.meta.url);
  if (!existsSync(new URL('index.html', dist))) return t.skip('dist fehlt');
  // Vite formt den Quelltext von classify um (Tabs); verglichen wird ohne Leerraum.
  const squash = (s) => s.replace(/\s+/g, '');
  assert.equal(squash(readFileSync(new URL('stage-class.js', dist), 'utf8')), squash(headClassScript()));
  for (const page of ['index.html', 'en/index.html']) {
    const html = readFileSync(new URL(page, dist), 'utf8');
    assert.match(html, /<script src="\/stage-class\.js"><\/script>/, page);
    const noscript = html.slice(html.indexOf('<noscript>'), html.indexOf('</noscript>'));
    for (const q of mediaOf(noscript)) assert.ok(Object.values(NOJS_MEDIA).includes(q), `${page}: @media ${q}`);
    // Die Schriften stehen vor dem ersten blockierenden Skript.
    assert.ok(html.indexOf('rel="preload"') < html.indexOf('<script src="/theme.js"'), page);
  }
  // Das gebündelte CSS kennt keine Query über das Seitenverhältnis mehr.
  for (const f of readdirSync(new URL('_astro/', dist)).filter((f) => f.endsWith('.css')))
    assert.doesNotMatch(readFileSync(new URL(`_astro/${f}`, dist), 'utf8'), /aspect-ratio/, f);
});
