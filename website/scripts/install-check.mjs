#!/usr/bin/env node
// Prüft nach dem Bau das Installationsskript der Website (HUM-528):
//   1. dist/install ist Byte für Byte install.sh aus dem Wurzelverzeichnis des Repositorys;
//   2. jede Leseseite (/installationsskript/ und /en/install-script/, die Wege aus
//      src/i18n/ui.ts) zeigt im hervorgehobenen Quelltext genau denselben Text.
//
//   node scripts/install-check.mjs [dist]
//
// Beendet sich mit Status 1 und nennt jede Abweichung, mit Status 2, wenn der Bau fehlt.
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

/** Die Leseseiten je Sprache, relativ zu dist; gleich den Wegen von `installer` in ui.ts. */
export const readPages = ['installationsskript/index.html', 'en/install-script/index.html'];

const entities = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'" };

/** Text eines HTML-Ausschnitts: Tags weg, Entitäten aufgelöst. */
export function htmlText(html) {
  return html
    .replace(/<[^>]*>/g, '')
    .replace(/&(#x[0-9a-f]+|#[0-9]+|[a-z]+);/gi, (whole, name) => {
      if (name[0] === '#') {
        const code = name[1] === 'x' || name[1] === 'X' ? parseInt(name.slice(2), 16) : parseInt(name.slice(1), 10);
        return String.fromCodePoint(code);
      }
      return entities[name.toLowerCase()] ?? whole;
    });
}

/** Der Quelltext im Block mit data-install-source, oder null, wenn die Seite keinen hat. */
export function shownSource(html) {
  const start = html.indexOf('data-install-source');
  if (start < 0) return null;
  const pre = html.slice(start).match(/<pre\b[^>]*>([\s\S]*?)<\/pre>/);
  return pre ? htmlText(pre[1]) : null;
}

/**
 * Alle Abweichungen zwischen install.sh, der ausgelieferten Datei und den Leseseiten.
 * @param {Buffer} script install.sh
 * @param {Buffer | null} served dist/install, null wenn sie fehlt
 * @param {{ name: string, html: string | null }[]} pages
 */
export function problems(script, served, pages) {
  const found = [];
  if (served === null) found.push('dist/install is missing');
  else if (!served.equals(script)) found.push(`dist/install differs from install.sh (${served.length} vs ${script.length} bytes)`);
  // Die Hervorhebung lässt den letzten Zeilenumbruch weg; sonst muss jedes Zeichen stimmen.
  const want = script.toString('utf8').replace(/\n$/, '');
  for (const page of pages) {
    if (page.html === null) {
      found.push(`${page.name} is missing`);
      continue;
    }
    const shown = shownSource(page.html);
    if (shown === null) found.push(`${page.name} shows no source block (data-install-source)`);
    else if (shown.replace(/\n$/, '') !== want) {
      const a = shown.split('\n');
      const b = want.split('\n');
      const line = a.findIndex((text, i) => text !== b[i]);
      found.push(`${page.name} shows a different text, first at line ${(line < 0 ? Math.min(a.length, b.length) : line) + 1}`);
    }
  }
  return found;
}

function main() {
  const here = path.dirname(fileURLToPath(import.meta.url));
  const dist = path.resolve(process.argv[2] ?? 'dist');
  if (!existsSync(dist)) {
    console.error(`install-check: ${dist} does not exist. Build the site first.`);
    process.exit(2);
  }
  const read = (file) => (existsSync(file) ? readFileSync(file) : null);
  const script = readFileSync(path.join(here, '..', '..', 'install.sh'));
  const served = read(path.join(dist, 'install'));
  const pages = readPages.map((name) => ({ name, html: read(path.join(dist, name))?.toString('utf8') ?? null }));
  const found = problems(script, served, pages);
  for (const problem of found) console.error(`install-check: ${problem}`);
  if (found.length > 0) process.exit(1);
  console.log(`install-check: /install equals install.sh (${script.length} bytes), ${pages.length} read pages show the same text`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
