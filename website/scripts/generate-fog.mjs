// Erzeugt die Nebelebenen der Startseite (HUM-341) aus Rauschen. Das Skript läuft von
// Hand, nicht im Build; das Ergebnis liegt in src/assets/fog/, die AVIF- und WebP-Fassungen
// baut Astro daraus. Der Startwert ist fest, zwei Läufe liefern dieselben Bilder.
//
//   node scripts/generate-fog.mjs
import { mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import sharp from 'sharp';

const out = join(dirname(fileURLToPath(import.meta.url)), '..', 'src', 'assets', 'fog');
const W = 1280;
const H = 720;
// Dieselbe Lichtrichtung wie LIGHT in src/scripts/stage.ts: von links oben, leicht von vorn.
const LIGHT = norm([-0.3, -0.85, 0.45]);

function norm([x, y, z]) {
  const l = Math.hypot(x, y, z);
  return [x / l, y / l, z / l];
}
const clamp = (v, a = 0, b = 1) => Math.min(b, Math.max(a, v));
const smooth = (a, b, v) => {
  const t = clamp((v - a) / (b - a));
  return t * t * (3 - 2 * t);
};

// Gitterrauschen mit fester Permutation, Summe über mehrere Oktaven.
const perm = Uint8Array.from({ length: 256 }, (_, i) => i);
for (let i = 255, s = 341; i > 0; i--) {
  s = (s * 1664525 + 1013904223) >>> 0;
  const j = s % (i + 1);
  [perm[i], perm[j]] = [perm[j], perm[i]];
}
const lattice = (x, y) => perm[(perm[x & 255] + y) & 255] / 255;
function noise(x, y) {
  const xi = Math.floor(x);
  const yi = Math.floor(y);
  const f = (t) => t * t * t * (t * (t * 6 - 15) + 10);
  const u = f(x - xi);
  const v = f(y - yi);
  const top = lattice(xi, yi) * (1 - u) + lattice(xi + 1, yi) * u;
  const bottom = lattice(xi, yi + 1) * (1 - u) + lattice(xi + 1, yi + 1) * u;
  return top * (1 - v) + bottom * v;
}
function fbm(x, y, octaves) {
  let sum = 0;
  let amp = 0.5;
  let all = 0;
  for (let o = 0; o < octaves; o++) {
    sum += amp * noise(x + o * 17.3, y - o * 9.1);
    all += amp;
    amp *= 0.5;
    x *= 2.03;
    y *= 2.03;
  }
  return sum / all;
}

// Dichte: verzerrtes Rauschen, damit Schwaden statt Flecken entstehen.
function density(x, y, scale, seed) {
  const sx = x * scale + seed;
  const sy = y * scale * 1.9 - seed;
  const wx = fbm(sx + 3.1, sy + 1.7, 3);
  const wy = fbm(sx - 2.3, sy + 5.9, 3);
  return fbm(sx + 1.9 * wx, sy + 1.3 * wy, 4);
}

// Beleuchtete Schwaden: die Dichte gilt als Höhenfeld, ihre Neigung zum Licht ergibt die
// Helligkeit. Daraus kommt der räumliche Eindruck, das Alpha trägt nur die Form.
function fogLayer({ scale, seed, low, high, relief, mask, peak, shadow = [120, 124, 134], lit: bright = [245, 248, 252] }) {
  const data = new Uint8Array(W * H * 4);
  const e = 1 / W;
  for (let y = 0; y < H; y++) {
    const v = y / H;
    for (let x = 0; x < W; x++) {
      const u = x / W;
      const d = density(u, v * (H / W), scale, seed);
      const dx = (density(u + 4 * e, v * (H / W), scale, seed) - d) * relief;
      const dy = (density(u, v * (H / W) + 4 * e, scale, seed) - d) * relief;
      const n = norm([-dx, -dy, 1]);
      const lit = smooth(0.1, 0.95, 0.5 + 0.5 * (n[0] * LIGHT[0] + n[1] * LIGHT[1] + n[2] * LIGHT[2]));
      const a = peak * smooth(low, high, d) * mask(u, v);
      const i = (y * W + x) * 4;
      data[i] = shadow[0] + (bright[0] - shadow[0]) * lit;
      data[i + 1] = shadow[1] + (bright[1] - shadow[1]) * lit;
      data[i + 2] = shadow[2] + (bright[2] - shadow[2]) * lit;
      data[i + 3] = 255 * a;
    }
  }
  return data;
}

// Lichtstrahlen fächern von einem Punkt oberhalb des Bildes entlang LIGHT nach rechts unten.
function raysLayer() {
  const data = new Uint8Array(W * H * 4);
  const sourceY = -0.75 * H;
  const sourceX = 0.5 * W + (LIGHT[0] / LIGHT[1]) * (0.5 * H - sourceY) * -1;
  const axis = Math.atan2(-LIGHT[1], -LIGHT[0]);
  const reach = 1.5 * H;
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const px = x - sourceX;
      const py = y - sourceY;
      const r = Math.hypot(px, py);
      const off = Math.atan2(py, px) - axis;
      const fine = fbm(off * 22 + 40, r * 0.0011, 2);
      const broad = fbm(off * 9 + 11, 2.5, 2);
      const shaft = smooth(0.4, 0.78, 0.6 * fine + 0.4 * broad);
      const fan = Math.exp(-((off / 0.33) ** 2));
      const fall = smooth(reach, 0.75 * H, r);
      const a = shaft * fan * fall * smooth(H, 0.62 * H, y);
      const i = (y * W + x) * 4;
      data[i] = 236;
      data[i + 1] = 240;
      data[i + 2] = 250;
      data[i + 3] = 255 * a;
    }
  }
  return data;
}

// Unter dem Text bleibt der Nebel dünn: die Dichte sitzt seitlich und zum Boden hin.
const sides = (u) => 0.45 + 0.55 * Math.abs(2 * u - 1);
const far = {
  scale: 2.4,
  seed: 3.7,
  low: 0.36,
  high: 0.72,
  relief: 30,
  peak: 0.85,
  mask: (u, v) => smooth(0.12, 0.5, v) * smooth(1, 0.7, v) * sides(u),
};
const near = {
  scale: 1.5,
  seed: 11.2,
  low: 0.34,
  high: 0.68,
  relief: 24,
  peak: 0.95,
  mask: (u, v) => smooth(0.56, 0.92, v) * (0.6 + 0.4 * sides(u)),
};
// Dieselben Schwaden für den hellen Raum (HUM-338): Schiefer statt Licht, sonst stünde ein
// grauer Schleier auf dem Papier. Die Strahlen bleiben weiß und gelten für beide Themen.
const slate = { shadow: [58, 66, 84], lit: [122, 132, 150] };
const layers = {
  far: fogLayer(far),
  'far-light': fogLayer({ ...far, ...slate }),
  near: fogLayer(near),
  'near-light': fogLayer({ ...near, ...slate }),
  rays: raysLayer(),
};

mkdirSync(out, { recursive: true });
for (const [name, data] of Object.entries(layers)) {
  const file = join(out, `${name}.webp`);
  const image = sharp(Buffer.from(data), { raw: { width: W, height: H, channels: 4 } });
  // Strahlen sind Streulicht: ohne Weichzeichnung lesen sie sich als Kratzer.
  if (name === 'rays') image.blur(9);
  const info = await image.webp({ quality: 92, alphaQuality: 95, effort: 6 }).toFile(file);
  console.log(`${name}.webp ${(info.size / 1024).toFixed(1)} KB`);
}
