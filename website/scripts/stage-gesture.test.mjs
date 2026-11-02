// Tests der Gesten des Karussells (src/scripts/stage/gesture.mjs): Senkrechtes Scrollen mit
// rauschendem deltaX dreht nie, eine waagrechte Geste bleibt waagrecht, nach einer Pause
// entscheidet die nächste Geste neu.
//
//   node --test scripts/
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { createWheelAxis, wheelPixels } from '../src/scripts/stage/gesture.mjs';

/** Senkrechter Trackpad-Wurf mit Nachlauf; deltaX rauscht um ±3 px. */
function verticalFling() {
  const events = [];
  for (let i = 0; i < 40; i++) {
    const dy = Math.max(0.5, 28 * Math.pow(0.9, i));
    const dx = (i % 3 === 0 ? 1 : -1) * Math.min(3, 0.8 + i * 0.08);
    events.push([dx, dy, i * 16]);
  }
  return events;
}

test('ein senkrechter Wurf mit Rauschen in deltaX bleibt ganz senkrecht', () => {
  const axis = createWheelAxis();
  const kinds = verticalFling().map(([dx, dy, t]) => axis(dx, dy, t));
  // Die alte Regel je Ereignis (|dx| > 1,2 |dy|) hätte gegen Ende mehrere Ereignisse gedreht.
  const oldRule = verticalFling().filter(([dx, dy]) => Math.abs(dx) > 1.2 * Math.abs(dy)).length;
  assert.ok(oldRule > 0, 'das Profil muss die alte Regel auslösen');
  assert.ok(!kinds.includes('x'), `Achsen: ${kinds.join(',')}`);
});

test('eine waagrechte Geste bleibt waagrecht, auch wenn deltaY kurz überwiegt', () => {
  const axis = createWheelAxis();
  const kinds = [];
  for (let i = 0; i < 20; i++) kinds.push(axis(i === 10 ? 1 : 14, i === 10 ? 6 : 2, i * 16));
  assert.equal(kinds.at(-1), 'x');
  assert.ok(kinds.slice(1).every((k) => k === 'x'), kinds.join(','));
});

test('nach einer Pause entscheidet die nächste Geste neu', () => {
  const axis = createWheelAxis();
  for (let i = 0; i < 5; i++) axis(14, 0, i * 16);
  assert.equal(axis(0, 20, 1000), 'y');
});

test('eine Zeilen-Maus (deltaMode 1) dreht schon mit der ersten Raste', () => {
  const axis = createWheelAxis();
  const notch = { deltaX: 3, deltaY: 0, deltaMode: 1 };
  // Roh gezählt läge eine Raste (3) unter der Schwelle von 8 px und würde verschluckt.
  assert.equal(createWheelAxis()(notch.deltaX, notch.deltaY, 0), null);
  const [dx, dy] = wheelPixels(notch, 1440);
  assert.deepEqual([dx, dy], [48, 0]);
  assert.equal(axis(dx, dy, 0), 'x');
  assert.deepEqual(wheelPixels({ deltaX: 0, deltaY: 1, deltaMode: 2 }, 800), [0, 800]);
});
