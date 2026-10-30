#!/usr/bin/env node
// Prüft die gebaute Seite auf tote interne Verweise (HUM-336): jeder Link auf eine Seite
// der Website muss auf eine vorhandene Datei zeigen, jeder Anker auf eine vorhandene ID
// (Überschriften und rohe <a id> oder <a name>). Dazu: kein relativer Link auf eine .md-Datei
// darf übrig bleiben, jedes lokale Bild muss existieren, und im Artikel einer Doc-Seite steht
// nur, was GitHub aus Markdown durchlässt (plus das Markup der Website: svg und path).
//
//   node scripts/check-links.mjs [dist]
//
// Beendet sich mit Status 1, sobald etwas fehlt, und nennt jede Fundstelle.
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

import { allowedTags } from '../src/lib/rehype-docs.mjs';

const dist = path.resolve(process.argv[2] ?? 'dist');
if (!existsSync(dist)) {
  console.error(`check-links: ${dist} does not exist. Build the site first.`);
  process.exit(2);
}

function htmlFiles(dir) {
  return readdirSync(dir).flatMap((name) => {
    const full = path.join(dir, name);
    if (statSync(full).isDirectory()) return htmlFiles(full);
    return name.endsWith('.html') ? [full] : [];
  });
}

const decode = (value) =>
  value
    .replace(/&amp;/g, '&')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&#x27;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>');

/** Ohne Codeblöcke, damit Beispiele in `pre` nicht als Links zählen. */
const withoutCode = (html) => html.replace(/<pre[\s\S]*?<\/pre>/g, '');

const pages = new Map();
for (const file of htmlFiles(dist)) {
  const rel = '/' + path.relative(dist, file).split(path.sep).join('/');
  const route = rel.endsWith('/index.html') ? rel.slice(0, -'index.html'.length) : rel;
  pages.set(route, { file, html: readFileSync(file, 'utf8') });
}

const idCache = new Map();
function idsOf(route) {
  if (!idCache.has(route)) {
    const html = pages.get(route).html;
    const ids = new Set();
    for (const match of html.matchAll(/\s(?:id|name)="([^"]*)"/g)) ids.add(decode(match[1]));
    idCache.set(route, ids);
  }
  return idCache.get(route);
}

const problems = [];
let checked = 0;

for (const [route, { html }] of pages) {
  const body = withoutCode(html);
  for (const match of body.matchAll(/<(a|img|link|script)\b[^>]*?\s(href|src)="([^"]*)"/g)) {
    const [, tag, attr, raw] = match;
    const url = decode(raw);
    if (/^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(url)) continue;
    checked += 1;
    if (tag === 'a' && /\.md(?:#|$)/i.test(url)) {
      problems.push(`${route}: link still points to a Markdown file: ${url}`);
      continue;
    }
    const resolved = new URL(url, `https://site${route}`);
    let target = decodeURIComponent(resolved.pathname);
    const hash = decodeURIComponent(resolved.hash.slice(1));
    if (tag !== 'a' || attr === 'src') {
      if (!existsSync(path.join(dist, target))) problems.push(`${route}: missing file for <${tag} ${attr}>: ${url}`);
      continue;
    }
    if (!pages.has(target)) {
      if (target.endsWith('/') || !existsSync(path.join(dist, target))) {
        problems.push(`${route}: dead link: ${url}`);
        continue;
      }
    }
    if (hash && pages.has(target) && !idsOf(target).has(hash)) {
      problems.push(`${route}: dead anchor: ${url}`);
    }
  }
}

const articleTags = new Set([...allowedTags, 'article', 'svg', 'path']);
for (const [route, { html }] of pages) {
  const article = /<article\b[^>]*class="[^"]*\bdoc\b[^"]*"[^>]*>([\s\S]*)<\/article>/.exec(html);
  if (!article) continue;
  const unexpected = new Set();
  for (const match of article[1].matchAll(/<([a-zA-Z][a-zA-Z0-9-]*)\b/g)) {
    const tag = match[1].toLowerCase();
    if (!articleTags.has(tag)) unexpected.add(tag);
  }
  for (const tag of unexpected) problems.push(`${route}: unexpected element <${tag}> in the article`);
}

if (problems.length) {
  console.error(`check-links: ${problems.length} problem(s) in ${pages.size} pages:`);
  for (const problem of problems) console.error(`  ${problem}`);
  process.exit(1);
}
console.log(`check-links: ${checked} internal links in ${pages.size} pages, all resolve.`);
