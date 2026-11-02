// Tests der Wache über das Video im vorderen Fenster (src/scripts/stage/watch.mjs, HUM-415):
// Ohne neues gemaltes Bild steht nach kurzer Zeit wieder die Aufnahme, nach Uhrzeit und ohne
// requestAnimationFrame; ein Hängen der Seite gilt nicht als Stillstand; ein leeres oder nicht
// prüfbares Bild öffnet das Gate nie; wiederholte Ausfälle lassen die Aufnahme stehen.
//
//   node --test scripts/
import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  FLAP_LIMIT,
  FLAP_WINDOW_MS,
  FRAME_FREEZE_MS,
  FREEZE_MS,
  GAP_MS,
  ON_LIMIT,
  PRE_OPEN_LIMIT,
  SEEK_GRACE_MS,
  WATCH_TICK_MS,
  createFlapGuard,
  createSampleJudge,
  isBlankFrame,
  judgeFreeze,
} from '../src/scripts/stage/watch.mjs';

/**
 * Läuft die Wache in festem Takt ab t = 0; frameAt(t) liefert true, wenn bis t ein Bild kam.
 * Liefert den Zeitpunkt des ersten "frozen" oder null.
 */
function run({ tick = WATCH_TICK_MS, until = 10000, frames = true, plannedSeek = false, frameAt, visible = () => true }) {
  let lastCheck = 0;
  let lastProgress = 0;
  for (let now = tick; now <= until; now += tick) {
    if (frameAt(now)) lastProgress = now;
    const v = judgeFreeze({ now, lastCheck, lastProgress, visible: visible(now), frames, plannedSeek });
    lastCheck = now;
    if (v === 'frozen') return now;
  }
  return null;
}

test('frames path: frozen within FRAME_FREEZE_MS + one tick of the last painted frame', () => {
  assert.ok(FRAME_FREEZE_MS >= 800 && FRAME_FREEZE_MS <= 1000);
  const at = run({ frameAt: (t) => t <= 1000 });
  assert.ok(at > 1000 + FRAME_FREEZE_MS && at <= 1000 + FRAME_FREEZE_MS + WATCH_TICK_MS, `${at}`);
});

test('frames path: sparse frames of the screen recording (gaps up to 600 ms) never freeze', () => {
  assert.equal(run({ tick: 50, frameAt: (t) => t % 600 === 0 }), null);
});

test('clock path (no frame callback): frozen after FREEZE_MS', () => {
  assert.equal(FREEZE_MS, 500);
  const at = run({ frames: false, frameAt: (t) => t <= 1000 });
  assert.ok(at > 1000 + FREEZE_MS && at <= 1000 + FREEZE_MS + WATCH_TICK_MS, `${at}`);
});

test('only the given progress counts: an advancing clock without frames is no progress', () => {
  // Der Aufrufer reicht mit Frame-Callback nur gemalte Bilder herein; ohne sie friert es ein,
  // auch wenn die Uhr läuft (das Szenario des grauen Fensters).
  const at = run({ frameAt: () => false });
  assert.ok(at !== null && at <= FRAME_FREEZE_MS + WATCH_TICK_MS, `${at}`);
});

test('wall time: a 1 s timer (throttled page) still freezes within ~2 s, not 6 s', () => {
  const at = run({ tick: 1000, frameAt: (t) => t <= 1000 });
  assert.ok(at !== null && at <= 1000 + FRAME_FREEZE_MS + 1000, `${at}`);
});

test('a hang of the page itself is not a stall; the next regular check decides', () => {
  // Prüfung bei 250, dann hängt die Seite bis 3250; die Bilder der Lücke kommen danach an.
  const base = { lastProgress: 250, frames: true, plannedSeek: false, visible: true };
  assert.equal(judgeFreeze({ ...base, now: 3250, lastCheck: 250 }), 'skip');
  assert.equal(judgeFreeze({ ...base, now: 3500, lastCheck: 3250, lastProgress: 3400 }), 'ok');
  // Kommen nach der Lücke keine Bilder, greift die nächste Prüfung.
  assert.equal(judgeFreeze({ ...base, now: 3500, lastCheck: 3250 }), 'frozen');
  assert.ok(GAP_MS > 1000 && GAP_MS < 2000);
});

test('only the first check after a gap is excused: throttled timers every 1.5 s still judge', () => {
  // Sichtbare Seite, kein rAF, Zeitgeber nur alle 1,5 s, keine Bilder: Die erste späte Prüfung
  // ist entschuldigt, die zweite urteilt, obwohl auch sie spät kommt.
  const base = { lastProgress: 0, frames: true, plannedSeek: false, visible: true };
  assert.equal(judgeFreeze({ ...base, now: 1500, lastCheck: 0 }), 'skip');
  assert.equal(judgeFreeze({ ...base, now: 3000, lastCheck: 1500, afterGap: true }), 'frozen');
  // Kamen in der Zwischenzeit Bilder, ist alles gut.
  assert.equal(judgeFreeze({ ...base, now: 3000, lastCheck: 1500, afterGap: true, lastProgress: 2800 }), 'ok');
});

test('a hidden page never counts as frozen', () => {
  assert.equal(run({ frameAt: () => false, visible: () => false }), null);
});

test('the own jump to the loop start gets a bounded grace, not forever', () => {
  const at = run({ plannedSeek: true, frameAt: () => false });
  assert.ok(at > SEEK_GRACE_MS && at <= SEEK_GRACE_MS + WATCH_TICK_MS, `${at}`);
  assert.ok(SEEK_GRACE_MS <= 2000);
});

/** RGBA-Pixel eines 32x18-Bildes aus einer Funktion (x, y) => [r, g, b, a]. */
function frame(fn, w = 32, h = 18) {
  const out = new Uint8ClampedArray(w * h * 4);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) out.set(fn(x, y), (y * w + x) * 4);
  return out;
}

test('a uniform grey, black or transparent frame is blank', () => {
  assert.equal(isBlankFrame(frame(() => [128, 128, 128, 255])), true);
  assert.equal(isBlankFrame(frame(() => [0, 0, 0, 255])), true);
  assert.equal(isBlankFrame(frame((x, y) => [60 + ((x + y) % 3), 60, 61, 255])), true);
  assert.equal(isBlankFrame(frame((x) => [x * 8, x * 8, x * 8, 0])), true);
  assert.equal(isBlankFrame(new Uint8ClampedArray(0)), true);
});

test('a frame of the recording (light text on dark ground) is not blank', () => {
  assert.equal(isBlankFrame(frame((x, y) => (y === 3 && x % 2 ? [220, 222, 228, 255] : [17, 18, 22, 255]))), false);
  assert.equal(isBlankFrame(frame((x, y) => (x === 5 && y === 5 ? [60, 60, 60, 255] : [17, 18, 22, 255]))), false);
});

test('before opening: only a verified frame opens; blank or unverified park after the limit', () => {
  assert.equal(PRE_OPEN_LIMIT, 3);
  for (const bad of ['blank', 'unverified']) {
    const j = createSampleJudge();
    assert.deepEqual([j.preOpen(bad), j.preOpen(bad), j.preOpen(bad)], ['wait', 'wait', 'park']);
  }
  const j = createSampleJudge();
  assert.equal(j.preOpen('ok'), 'open');
  // Ein echtes Bild setzt die Folge zurück.
  assert.deepEqual([j.preOpen('unverified'), j.preOpen('blank'), j.preOpen('ok'), j.preOpen('blank'), j.preOpen('blank')], ['wait', 'wait', 'open', 'wait', 'wait']);
});

test('while on: two blank or unverified samples in a row fail, one alone does not', () => {
  assert.equal(ON_LIMIT, 2);
  const j = createSampleJudge();
  assert.deepEqual([j.whileOn('blank'), j.whileOn('ok'), j.whileOn('unverified'), j.whileOn('blank')], ['ok', 'ok', 'ok', 'fail']);
  j.reset();
  assert.equal(j.whileOn('blank'), 'ok');
});

test('repeated failures park the video, then stop it for the visit', () => {
  const g = createFlapGuard();
  const verdicts = [];
  for (let i = 0; i < FLAP_LIMIT; i++) verdicts.push(g.note(1000 + i * 1000));
  assert.deepEqual(verdicts, ['retry', 'retry', 'park']);
  g.gesture();
  assert.equal(g.note(10000), 'retry');
  assert.equal(g.note(11000), 'retry');
  assert.equal(g.note(12000), 'stop');
});

test('a direct park (blank before opening) counts toward stop', () => {
  const g = createFlapGuard();
  assert.equal(g.park(), 'park');
  g.gesture();
  assert.equal(g.park(), 'stop');
});

test('failures far apart (the loop jump every 16.5 s) never park', () => {
  const g = createFlapGuard();
  for (let i = 0; i < 20; i++) assert.equal(g.note(i * 16500), 'retry');
  assert.ok(16500 * (FLAP_LIMIT - 1) >= FLAP_WINDOW_MS);
});

test('a gesture clears the count', () => {
  const g = createFlapGuard();
  g.note(0);
  g.note(100);
  g.gesture();
  assert.equal(g.note(200), 'retry');
});
