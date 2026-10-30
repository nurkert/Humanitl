#!/usr/bin/env node
// Stand der Übersetzungen der Docs (HUM-346): welche Datei unter docs/ hat in der anderen
// Sprache keine Übersetzung, welche Übersetzung beruht auf einer älteren Fassung ihres
// Originals (source-sha256 passt nicht mehr), welche hat eine andere Gliederung als das
// Original (dann verliert die Website die gemeinsamen Anker), welche hat kein Original mehr
// oder kaputtes Front Matter.
//
//   node scripts/check-translations.mjs [--json]
//
// Nur ein Bericht: beendet sich immer mit Status 0, auch wenn sich eine Datei nicht lesen
// lässt (dann steht sie unter „Unreadable"). Andere Sitzungen ändern die Docs laufend, und die
// Website zeigt fehlende und veraltete Übersetzungen mit einem Hinweis an.
import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

import { docId, docsRoot, originalLang, translationsRoot } from '../src/lib/docs-catalog.mjs';

const json = process.argv.includes('--json');

// Was sich nicht lesen lässt, landet hier statt in einem Abbruch.
const unreadable = [];
const repoPath = (full) => path.relative(path.dirname(docsRoot), full).split(path.sep).join('/');
const describe = (error) => error?.code ?? error?.message ?? String(error);

function markdownFiles(dir, base = dir) {
  let names;
  try {
    if (!existsSync(dir)) return [];
    names = readdirSync(dir).sort();
  } catch (error) {
    unreadable.push({ file: repoPath(dir), error: describe(error) });
    return [];
  }
  return names.flatMap((name) => {
    const full = path.join(dir, name);
    try {
      if (statSync(full).isDirectory()) return full === translationsRoot ? [] : markdownFiles(full, base);
    } catch (error) {
      unreadable.push({ file: repoPath(full), error: describe(error) });
      return [];
    }
    return name.endsWith('.md') ? [path.relative(base, full).split(path.sep).join('/')] : [];
  });
}

/** Inhalt einer Datei oder `null`; der Fehler steht dann unter „Unreadable". */
function read(full, encoding) {
  try {
    return readFileSync(full, encoding);
  } catch (error) {
    unreadable.push({ file: repoPath(full), error: describe(error) });
    return null;
  }
}

/** Front Matter am Anfang: einfache Zeilen `schlüssel: wert`. */
function frontMatter(text) {
  const match = /^\uFEFF?---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/.exec(text);
  if (!match) return { data: null, body: text };
  const data = {};
  for (const line of match[1].split(/\r?\n/)) {
    const pair = /^([A-Za-z0-9_-]+):\s*(.*?)\s*$/.exec(line);
    if (pair) data[pair[1]] = pair[2];
  }
  return { data, body: text.slice(match[0].length) };
}

/** Ebenen der ATX-Überschriften außerhalb von Codeblöcken, in Reihenfolge. */
function headingLevels(markdown) {
  const levels = [];
  let fence = null;
  for (const line of markdown.split(/\r?\n/)) {
    const open = /^\s{0,3}(`{3,}|~{3,})/.exec(line);
    if (open) {
      if (!fence) fence = open[1][0];
      else if (open[1][0] === fence) fence = null;
      continue;
    }
    if (fence) continue;
    const heading = /^\s{0,3}(#{1,6})(?:\s|$)/.exec(line);
    if (heading) levels.push(heading[1].length);
  }
  return levels;
}

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

function main() {
  const originals = markdownFiles(docsRoot);
  const translations = ['de', 'en'].flatMap((lang) =>
    markdownFiles(path.join(translationsRoot, lang)).map((rel) => ({ lang, rel })),
  );

  const report = { ok: [], missing: [], stale: [], structure: [], orphan: [], invalid: [] };
  const byKey = new Map();

  for (const { lang, rel } of translations) {
    const file = `docs/translations/${lang}/${rel}`;
    const text = read(path.join(translationsRoot, lang, rel), 'utf8');
    if (text === null) continue;
    const { data, body } = frontMatter(text);
    const problems = [];
    if (!data) problems.push('no front matter');
    else {
      if (!data.source) problems.push('source missing');
      if (!/^[0-9a-f]{64}$/.test(data['source-sha256'] ?? '')) problems.push('source-sha256 missing or malformed');
      if (data.lang !== lang) problems.push(`lang is "${data.lang ?? ''}", folder says "${lang}"`);
      if (data.source && data.source !== rel) problems.push(`source is ${data.source}, path says ${rel}`);
    }
    if (problems.length) {
      report.invalid.push({ file, problems });
      continue;
    }
    const sourceFile = path.join(docsRoot, data.source);
    let present = false;
    try {
      present = statSync(sourceFile).isFile();
    } catch {
      present = false;
    }
    if (!present) {
      report.orphan.push({ file, source: `docs/${data.source}` });
      continue;
    }
    const id = docId(data.source);
    if (originalLang(id) === lang) {
      report.invalid.push({ file, problems: [`docs/${data.source} is already written in ${lang}`] });
      continue;
    }
    byKey.set(`${lang}:${id}`, file);
    const entry = { file, source: `docs/${data.source}` };
    const sourceBytes = read(sourceFile);
    if (sourceBytes === null) continue;
    const theirs = headingLevels(sourceBytes.toString('utf8'));
    const own = headingLevels(body);
    if (own.join() !== theirs.join()) {
      const at = own.findIndex((level, index) => level !== theirs[index]);
      report.structure.push({
        ...entry,
        detail:
          own.length !== theirs.length
            ? `${own.length} headings, the original has ${theirs.length}`
            : `heading ${at + 1} is h${own[at]}, in the original h${theirs[at]}`,
      });
    }
    if (sha256(sourceBytes) !== data['source-sha256']) report.stale.push(entry);
    else report.ok.push(entry);
  }

  for (const rel of originals) {
    const id = docId(rel);
    const target = originalLang(id) === 'de' ? 'en' : 'de';
    if (!byKey.has(`${target}:${id}`)) report.missing.push({ source: `docs/${rel}`, lang: target });
  }

  report.unreadable = unreadable;
  if (json) {
    console.log(JSON.stringify(report, null, 2));
    return;
  }

  const section = (title, rows, line) => {
    if (!rows.length) return;
    console.log(`\n${title} (${rows.length}):`);
    for (const row of rows) console.log(`  ${line(row)}`);
  };
  section('Missing translations', report.missing, (row) => `${row.source} -> docs/translations/${row.lang}/${row.source.slice(5)}`);
  section('Stale translations (original changed since)', report.stale, (row) => `${row.file} (original ${row.source})`);
  section('Heading structure differs (site falls back to own anchors)', report.structure, (row) => `${row.file}: ${row.detail}`);
  section('Translations without an original', report.orphan, (row) => `${row.file} (expected ${row.source})`);
  section('Invalid translations (ignored by the site)', report.invalid, (row) => `${row.file}: ${row.problems.join('; ')}`);
  section('Unreadable files or folders', report.unreadable, (row) => `${row.file}: ${row.error}`);

  const total = originals.length;
  console.log(
    `\ncheck-translations: ${total} docs, ${report.ok.length} translations current, ${report.stale.length} stale, ` +
      `${report.missing.length} missing, ${report.structure.length} with a different structure, ` +
      `${report.orphan.length} orphaned, ${report.invalid.length} invalid, ${report.unreadable.length} unreadable. ` +
      'Informational only.',
  );
}

try {
  main();
} catch (error) {
  console.error(`check-translations: report incomplete: ${error?.stack ?? error}`);
}
process.exit(0);
