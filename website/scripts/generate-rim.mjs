// Die vier Eckkacheln der Glaskante (HUM-375): Das Band der Fase läuft in .win__rim als Maske
// um das Fenster, und ein CSS-Verlauf kennt in den Ecken nur Kreise um eine Mitte. Die Kante hat
// aber den viel kleineren Radius des Rahmens. Hier ist die Tiefe darum der Abstand zum echten
// gerundeten Rand; Kurve und Verhältnis stehen in Home.astro (--bevel, --r, --b).
import { mkdirSync, writeFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';

const SIZE = 192;
const RADIUS = 0.016 / 0.05;
const STOPS = [
  [0, 0],
  [0.22, 0.05],
  [0.44, 0.2],
  [0.64, 0.5],
  [0.83, 1],
  [0.93, 0.6],
  [1, 0.12],
];

function curve(t) {
  for (let i = 1; i < STOPS.length; i++) {
    const [a, va] = STOPS[i - 1];
    const [b, vb] = STOPS[i];
    if (t <= b) return va + ((vb - va) * (t - a)) / (b - a);
  }
  return STOPS.at(-1)[1];
}

/** Die Tiefe von (x, y) unter dem Rand, beide von der Fensterecke aus, in Kachelbreiten. */
function depth(x, y) {
  if (x < RADIUS && y < RADIUS) return RADIUS - Math.hypot(RADIUS - x, RADIUS - y);
  return Math.min(x, y);
}

function crc32(bytes) {
  let crc = ~0;
  for (const byte of bytes) {
    crc ^= byte;
    for (let k = 0; k < 8; k++) crc = crc & 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
  }
  return ~crc >>> 0;
}

function chunk(type, data) {
  const body = Buffer.concat([Buffer.from(type), data]);
  const out = Buffer.alloc(body.length + 8);
  out.writeUInt32BE(data.length, 0);
  body.copy(out, 4);
  out.writeUInt32BE(crc32(body), body.length + 4);
  return out;
}

function png(alpha) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(SIZE, 0);
  header.writeUInt32BE(SIZE, 4);
  header[8] = 8;
  header[9] = 4;
  const raw = Buffer.alloc((SIZE * 2 + 1) * SIZE);
  for (let y = 0; y < SIZE; y++) {
    const row = y * (SIZE * 2 + 1);
    for (let x = 0; x < SIZE; x++) raw[row + 2 + x * 2] = alpha(x, y);
  }
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** Viermal abgetastet, damit der äußere Bogen nicht treppt. */
function coverage(x, y) {
  let sum = 0;
  for (const dy of [0.25, 0.75]) {
    for (const dx of [0.25, 0.75]) {
      const d = depth((x + dx) / SIZE, (y + dy) / SIZE);
      if (d >= 0) sum += curve(1 - Math.min(d, 1));
    }
  }
  return Math.round((sum / 4) * 255);
}

const out = new URL('../public/rim/', import.meta.url);
mkdirSync(out, { recursive: true });
const corners = { tl: [false, false], tr: [true, false], bl: [false, true], br: [true, true] };
for (const [name, [flipX, flipY]] of Object.entries(corners)) {
  const flip = (v, on) => (on ? SIZE - 1 - v : v);
  writeFileSync(new URL(`${name}.png`, out), png((x, y) => coverage(flip(x, flipX), flip(y, flipY))));
}
