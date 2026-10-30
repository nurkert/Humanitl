#!/usr/bin/env node
// Prüft die gebaute Seite auf tote interne Verweise (HUM-336): jeder Link auf eine Seite
// der Website muss auf eine vorhandene Datei zeigen, jeder Anker auf eine vorhandene ID
// (Überschriften und rohe <a id> oder <a name>). Dazu: kein relativer Link auf eine .md-Datei
// darf übrig bleiben, jedes lokale Bild muss existieren, und im Artikel einer Doc-Seite steht
// nur, was GitHub aus Markdown durchlässt (plus das Markup der Website: svg und path).
//
// Seit HUM-346 gibt es die Docs in zwei Sprachen, deutsch unter /docs/, englisch unter
// /en/docs/. Dazu wird geprüft: jede Seite hat ihr Gegenstück in der anderen Sprache, trägt
// das passende <html lang>, nennt beide Fassungen als hreflang und in der Sprachwahl, und ihr
// Artikel verweist nur auf Docs-Seiten derselben Sprache. `<html lang>` nennt die Sprache des
// gezeigten Textes: die Sprache des Wegs, außer eine Seite ohne Übersetzung zeigt das Original
// (Hinweis „noch nicht übersetzt"); dann die Sprache des Artikels.
//
// Ein toter Anker in eine Seite, deren Übersetzung veraltet ist oder anders gegliedert als ihr
// Original (Artikel mit data-stale oder data-heading-fallback), ist nur eine Warnung: eine
// veraltete Übersetzung darf den Bau nie anhalten (HUM-346).
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

const articleOf = (html) => /<article\b[^>]*class="[^"]*\bdoc\b[^"]*"[^>]*>/.exec(html)?.[0] ?? '';
/** Seiten, deren Anker sich mit dem Original verschoben haben können. */
const tolerant = (route) => /\sdata-(?:stale|heading-fallback)\b/.test(articleOf(pages.get(route).html));

const problems = [];
const warnings = [];
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
      (tolerant(target) ? warnings : problems).push(`${route}: dead anchor: ${url}`);
    }
  }
}

// Zwei Sprachen der Docs (HUM-346).
const docTrees = { de: '/docs/', en: '/en/docs/' };
const docLangOf = (route) => (route.startsWith(docTrees.en) ? 'en' : route.startsWith(docTrees.de) ? 'de' : null);
const counterpart = (route, lang) => {
  const from = docLangOf(route);
  return docTrees[lang] + route.slice(docTrees[from].length);
};
let docPages = 0;
for (const [route, { html }] of pages) {
  const lang = docLangOf(route);
  if (!lang || !route.endsWith('/')) continue;
  docPages += 1;
  const htmlLang = /<html\b[^>]*\slang="([^"]*)"/.exec(html)?.[1];
  const articleTag = articleOf(html);
  const articleLang = /\slang="([^"]*)"/.exec(articleTag)?.[1];
  const missing = /\sdata-version="missing"/.test(articleTag);
  // Ohne Übersetzung zeigt die Seite das Original; sonst Text in der Sprache des Wegs.
  const textLang = articleTag && missing ? articleLang : lang;
  if (articleTag && articleLang !== textLang) {
    problems.push(`${route}: <article lang="${articleLang}">, expected "${textLang}"`);
  }
  if (htmlLang !== textLang) problems.push(`${route}: <html lang="${htmlLang}">, expected "${textLang}"`);
  if (missing && !html.includes('data-notice="missing"')) problems.push(`${route}: shows the original without the notice`);
  const head = html.slice(0, html.indexOf('</head>'));
  const alternates = new Map();
  for (const match of head.matchAll(/<link\b[^>]*\brel="alternate"[^>]*>/g)) {
    const hreflang = /\shreflang="([^"]*)"/.exec(match[0])?.[1];
    const href = /\shref="([^"]*)"/.exec(match[0])?.[1];
    if (hreflang && href) alternates.set(hreflang, decodeURIComponent(new URL(decode(href)).pathname));
  }
  for (const other of Object.keys(docTrees)) {
    const expected = counterpart(route, other);
    if (!pages.has(expected)) problems.push(`${route}: no ${other} counterpart ${expected}`);
    if (alternates.get(other) !== expected) {
      problems.push(`${route}: hreflang="${other}" is ${alternates.get(other) ?? 'missing'}, expected ${expected}`);
    }
  }
  if (alternates.get('x-default') !== counterpart(route, 'de')) {
    problems.push(`${route}: hreflang="x-default" should name the German page`);
  }
  const switcher = /<ul class="nav__lang"[^>]*>([\s\S]*?)<\/ul>/.exec(html)?.[1] ?? '';
  for (const other of Object.keys(docTrees)) {
    const expected = counterpart(route, other);
    if (!switcher.includes(`href="${expected}"`)) problems.push(`${route}: language switch lacks ${expected}`);
  }
  const article = /<article\b[^>]*class="[^"]*\bdoc\b[^"]*"[^>]*>([\s\S]*)<\/article>/.exec(html)?.[1];
  if (!article) continue;
  for (const match of withoutCode(article).matchAll(/\shref="([^"]*)"/g)) {
    const target = decode(match[1]);
    const targetLang = target.startsWith('/') ? docLangOf(target) : null;
    if (targetLang && targetLang !== lang) problems.push(`${route}: article links into the ${targetLang} docs: ${target}`);
  }
}
if (docPages === 0) problems.push('no docs pages found under /docs/ or /en/docs/');

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

if (warnings.length) {
  console.warn(`check-links: ${warnings.length} warning(s), stale or differently structured translations:`);
  for (const warning of warnings) console.warn(`  ${warning}`);
}
if (problems.length) {
  console.error(`check-links: ${problems.length} problem(s) in ${pages.size} pages:`);
  for (const problem of problems) console.error(`  ${problem}`);
  process.exit(1);
}
console.log(
  `check-links: ${checked} internal links in ${pages.size} pages, all resolve; ${docPages} docs pages in de and en, all paired.`,
);
