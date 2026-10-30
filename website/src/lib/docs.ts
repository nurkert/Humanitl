// Ordnung und Beschriftung der Docs-Seiten (HUM-336), in beiden Sprachen der Website
// (HUM-346). Die Gruppen folgen docs/README.md; Dateien, die hier fehlen, landen in „More",
// damit keine Datei ohne Seite bleibt.
//
// Jede Datei unter docs/ ist eine Seite in jeder Sprache. Welche Fassung sie zeigt:
// - die Sprache des Originals: das Original;
// - die andere Sprache: die Übersetzung aus docs/translations/<lang>/, sonst das Original mit
//   dem Hinweis „noch nicht übersetzt". Passt `source-sha256` nicht mehr zum Original, zeigt
//   die Seite die Übersetzung mit dem Hinweis, dass sich das Original seither geändert hat.
// Nichts davon hält den Bau an; scripts/check-translations.mjs listet fehlende und veraltete.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';

import type { CollectionEntry } from 'astro:content';

import { docHref, docId, docLangs, githubBlob, originalLang, repoUrl } from './docs-catalog.mjs';

export { docHref, repoUrl };

export type DocLang = 'de' | 'en';
export type DocEntry = CollectionEntry<'docs'>;
export type TranslationEntry = CollectionEntry<'translations'>;
/** Was eine Seite rendert: ein Original oder eine Übersetzung. */
export type RenderedEntry = DocEntry | TranslationEntry;

interface GroupSpec {
  key: string;
  /** Feste Reihenfolge mit kurzem Label je Sprache; `null` nimmt den Titel der Datei. */
  items: [id: string, label: Record<DocLang, string> | null][];
}

const groupSpecs: GroupSpec[] = [
  { key: 'start', items: [['readme', { en: 'Overview', de: 'Überblick' }]] },
  {
    key: 'using',
    items: [
      ['guides/install', { en: 'Install', de: 'Installation' }],
      ['guides/first-session', { en: 'First session', de: 'Erste Session' }],
      ['guides/agent-profiles', { en: 'Agent profiles', de: 'Agentenprofile' }],
      ['profiles', { en: 'Profiles (stub)', de: 'Profile (Platzhalter)' }],
    ],
  },
  {
    key: 'reference',
    items: [
      ['reference/rules', { en: 'Rules', de: 'Regeln' }],
      ['reference/cli', { en: 'CLI reference', de: 'CLI-Referenz' }],
      ['config', { en: 'Configuration', de: 'Konfiguration' }],
      ['diagnostics', { en: 'Diagnostic codes', de: 'Diagnose-Codes' }],
      ['reference/limits', { en: 'Resource limits', de: 'Ressourcengrenzen' }],
      ['reference/parity', { en: 'RPC parity', de: 'RPC-Parität' }],
      ['cli', { en: 'CLI walkthrough', de: 'CLI Schritt für Schritt' }],
      ['protocol', { en: 'Protocol', de: 'Protokoll' }],
      ['glossary', { en: 'Glossary', de: 'Glossar' }],
    ],
  },
  {
    key: 'security',
    items: [
      ['security', { en: 'Security', de: 'Sicherheit' }],
      ['threat-model', { en: 'Threat model', de: 'Bedrohungsmodell' }],
    ],
  },
  {
    key: 'building',
    items: [
      ['architecture', { en: 'Architecture', de: 'Architektur' }],
      ['design', { en: 'Design system', de: 'Designsystem' }],
      ['ux', { en: 'UX guidelines', de: 'UX-Richtlinien' }],
      ['install', { en: 'Maintainer notes', de: 'Hinweise für Paketbetreuer' }],
    ],
  },
  { key: 'decisions', items: [['adr/readme', { en: 'All decisions', de: 'Alle Entscheidungen' }]] },
];

const templateLabel: Record<DocLang, string> = { en: 'Template', de: 'Vorlage' };

// ------------------------------------------------------------------ Fassungen je Sprache

export type VersionKind = 'original' | 'translation' | 'missing';

export interface DocVersion {
  id: string;
  /** Sprache der Seite (des Wegs). */
  lang: DocLang;
  /** Sprache des Textes, den die Seite zeigt. */
  contentLang: DocLang;
  originalLang: DocLang;
  kind: VersionKind;
  /** Die Übersetzung beruht auf einer älteren Fassung des Originals. */
  stale: boolean;
  entry: RenderedEntry;
  original: DocEntry;
  /** Pfad der gezeigten Datei ab der Wurzel des Repositorys. */
  file: string;
  /** Pfad des Originals ab der Wurzel des Repositorys. */
  source: string;
}

interface TranslationData {
  source?: unknown;
  'source-sha256'?: unknown;
  lang?: unknown;
}

export interface TranslationIndex {
  get(lang: DocLang, id: string): TranslationEntry | undefined;
}

const warned = new Set<string>();
function warnOnce(message: string) {
  if (warned.has(message)) return;
  warned.add(message);
  console.warn(`[docs] ${message}`);
}

/** Die Übersetzungen nach Sprache und ID des Originals; ungültige werden mit Warnung übergangen. */
export function translationIndex(translations: TranslationEntry[]): TranslationIndex {
  const map = new Map<string, TranslationEntry>();
  for (const entry of translations) {
    const data = (entry.data ?? {}) as TranslationData;
    const file = translationFile(entry);
    if (typeof data.source !== 'string' || typeof data['source-sha256'] !== 'string') {
      warnOnce(`${file}: front matter needs source and source-sha256; skipped.`);
      continue;
    }
    if (!docLangs.includes(data.lang as string)) {
      warnOnce(`${file}: front matter lang must be de or en; skipped.`);
      continue;
    }
    // Wie check-translations.mjs („invalid"): Sprache und Original folgen aus dem Pfad der
    // Datei; widerspricht das Front Matter, gilt die Übersetzung als ungültig.
    const where = /^docs\/translations\/([^/]+)\/(.+)$/.exec(file);
    if (!where || where[1] !== data.lang) {
      warnOnce(`${file}: front matter lang is "${data.lang}", the folder says "${where?.[1] ?? '?'}"; skipped.`);
      continue;
    }
    if (where[2] !== data.source) {
      warnOnce(`${file}: front matter source is ${data.source}, the path says ${where[2]}; skipped.`);
      continue;
    }
    map.set(`${data.lang}:${docId(data.source)}`, entry);
  }
  return { get: (lang, id) => map.get(`${lang}:${id}`) };
}

const hashes = new Map<string, string | null>();
/**
 * SHA-256 der Bytes eines Originals, so wie die Übersetzer ihn bilden (`shasum -a 256`).
 * Gelesen über `filePath` des Eintrags, den der Glob-Loader relativ zur Wurzel des
 * Astro-Projekts angibt; `docsRoot` taugt hier nicht, weil diese Datei beim Bau gebündelt
 * wird und `import.meta.url` dann auf den Bündel zeigt. Lässt sich die Datei nicht lesen,
 * gilt die Übersetzung als aktuell, damit kein Hinweis auf Verdacht erscheint.
 */
function sourceHash(original: DocEntry): string | null {
  const key = original.filePath ?? original.id;
  if (!hashes.has(key)) {
    try {
      const file = path.resolve(process.cwd(), original.filePath ?? '');
      hashes.set(key, createHash('sha256').update(readFileSync(file)).digest('hex'));
    } catch {
      warnOnce(`${sourcePath(original)}: not readable from ${process.cwd()}; translations of it count as current.`);
      hashes.set(key, null);
    }
  }
  return hashes.get(key)!;
}

function relativeInDocs(file: string | undefined, fallback: string): string {
  const normal = file?.replace(/\\/g, '/') ?? '';
  return normal.includes('docs/') ? normal.slice(normal.lastIndexOf('docs/') + 5) : fallback;
}

export function sourcePath(entry: DocEntry): string {
  return `docs/${relativeInDocs(entry.filePath, `${entry.id}.md`)}`;
}

function translationFile(entry: TranslationEntry): string {
  return `docs/${relativeInDocs(entry.filePath, `translations/${entry.id}.md`)}`;
}

/** Die Fassung, die die Seite `id` in der Sprache `lang` zeigt. */
export function resolveVersion(original: DocEntry, lang: DocLang, translations: TranslationIndex): DocVersion {
  const id = original.id;
  const origin = originalLang(id) as DocLang;
  const source = sourcePath(original);
  const base = { id, lang, originalLang: origin, original, source };
  if (lang === origin) {
    return { ...base, contentLang: origin, kind: 'original', stale: false, entry: original, file: source };
  }
  const translation = translations.get(lang, id);
  if (!translation) {
    return { ...base, contentLang: origin, kind: 'missing', stale: false, entry: original, file: source };
  }
  const data = translation.data as TranslationData;
  const current = sourceHash(original);
  return {
    ...base,
    contentLang: lang,
    kind: 'translation',
    stale: current !== null && current !== data['source-sha256'],
    entry: translation,
    file: translationFile(translation),
  };
}

// ------------------------------------------------------------------ Text aus dem Markdown

export interface DocItem {
  id: string;
  href: string;
  label: string;
  /** Nummer einer ADR („0001"), sonst leer. */
  number: string;
  title: string;
  summary: string;
  /** Pfad des Originals ab der Wurzel des Repositorys. */
  source: string;
  /** Pfad der gezeigten Datei (Original oder Übersetzung). */
  file: string;
  lang: DocLang;
  contentLang: DocLang;
  kind: VersionKind;
  stale: boolean;
  generated: boolean;
  status: string;
  groupKey: string;
}

export interface DocGroup {
  key: string;
  items: DocItem[];
}

const stripMarks = (value: string) => value.replace(/[*_`]/g, '').trim();

/** Die erste H1 ist der Titel, wie auf GitHub. */
export function docTitle(entry: RenderedEntry): string {
  const match = entry.body?.match(/^#\s+(.+)$/m);
  return match ? match[1].trim() : entry.id;
}

export function plainTitle(entry: RenderedEntry): string {
  return stripMarks(docTitle(entry));
}

const metaLine = /^(Applies to|Status|Date):/;

/** Inline-Markdown grob zu Text: Links behalten ihren Text, Zeichen für Hervorhebung fallen weg. */
function plain(value: string): string {
  // Inline-Code zuerst beiseitelegen, damit `profiles/*.toml` sein Sternchen behält.
  const codes: string[] = [];
  return value
    .replace(/`([^`]*)`/g, (_, code: string) => `\u0000${codes.push(code) - 1}\u0000`)
    .replace(/!\[[^\]]*\]\([^)]*\)/g, '')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/(\*\*|__|\*|_)(?=\S)([^*_]+?)\1/g, '$2')
    .replace(/\u0000(\d+)\u0000/g, (_, index: string) => codes[Number(index)])
    .replace(/\s+/g, ' ')
    .trim();
}

function clip(value: string, max = 180): string {
  return value.length > max ? `${value.slice(0, max - 1).replace(/\s+\S*$/, '')}…` : value;
}

/** Beschreibung einer Seite: das Blockzitat „Purpose:", sonst der erste echte Absatz. */
export function docSummary(entry: RenderedEntry): string {
  const lines = (entry.body ?? '').split('\n');
  const purpose = lines.findIndex((line) => /^>\s*(Purpose|Zweck):/.test(line));
  if (purpose !== -1) {
    const block: string[] = [];
    for (let i = purpose; i < lines.length && lines[i].startsWith('>'); i += 1) block.push(lines[i].replace(/^>\s?/, ''));
    return clip(plain(block.join(' ').replace(/^(Purpose|Zweck):\s*/, '')));
  }
  let inFence = false;
  const paragraph: string[] = [];
  for (const line of lines) {
    if (/^\s*(```|~~~)/.test(line)) inFence = !inFence;
    if (inFence) continue;
    const trimmed = line.trim();
    if (!trimmed) {
      if (paragraph.length) break;
      continue;
    }
    if (/^(#|<!--|\||>|-|\*\s|\d+\.|!\[)/.test(trimmed) || metaLine.test(trimmed)) {
      if (paragraph.length) break;
      continue;
    }
    paragraph.push(trimmed);
  }
  return clip(plain(paragraph.join(' ')));
}

/** Die Spalte „What it answers" aus docs/README.md (oder ihrer Übersetzung), nachgeschlagen über das Linkziel. */
function readmeSummaries(readme: RenderedEntry | undefined): Map<string, string> {
  const map = new Map<string, string>();
  for (const line of (readme?.body ?? '').split('\n')) {
    const match = /^\|\s*\[[^\]]*\]\(([^)#]+)[^)]*\)\s*\|\s*(.+?)\s*\|\s*$/.exec(line);
    if (!match) continue;
    const id = match[1].replace(/\.md$/i, '').toLowerCase();
    const text = plain(match[2]);
    map.set(id, text.charAt(0).toUpperCase() + text.slice(1));
  }
  return map;
}

export const blobUrl = (file: string) => `${githubBlob}${file}`;
export const editUrl = (file: string) => `${repoUrl}/edit/main/${file}`;
export const historyUrl = (file: string) => `${repoUrl}/commits/main/${file}`;

export function isGenerated(entry: RenderedEntry): boolean {
  return /^<!--\s*(?:Erzeugt|Generated)\b/m.test((entry.body ?? '').split('\n').slice(0, 12).join('\n'));
}

function status(entry: RenderedEntry): string {
  return entry.body?.match(/^Status:\s*(.+)$/m)?.[1].trim() ?? '';
}

// ------------------------------------------------------------------ Gruppen

/** Alle Seiten einer Sprache in Gruppen, in der Reihenfolge der Seitenleiste. */
export function docGroups(entries: DocEntry[], translations: TranslationEntry[], lang: DocLang): DocGroup[] {
  const index = translationIndex(translations);
  const byId = new Map(entries.map((entry) => [entry.id, entry]));
  const readme = byId.get('readme');
  // Die Spalte der README nur, wenn die README in der Sprache der Seite vorliegt.
  const readmeVersion = readme ? resolveVersion(readme, lang, index) : undefined;
  const summaries = readmeSummaries(readmeVersion?.contentLang === lang ? readmeVersion.entry : undefined);
  const used = new Set<string>();

  const item = (entry: DocEntry, label: Record<DocLang, string> | null, groupKey: string): DocItem => {
    used.add(entry.id);
    const version = resolveVersion(entry, lang, index);
    const title = plainTitle(version.entry);
    const adr = /^ADR-(\d{4}|NNNN)\s*·\s*(.+)$/.exec(title);
    let number = '';
    let shortLabel = label?.[lang] ?? title;
    if (!label && adr) {
      number = adr[1] === 'NNNN' ? '0000' : adr[1];
      shortLabel = adr[1] === 'NNNN' ? templateLabel[lang] : adr[2];
    }
    return {
      id: entry.id,
      href: docHref(entry.id, lang),
      label: shortLabel,
      number,
      title,
      summary: summaries.get(entry.id) ?? docSummary(version.entry),
      source: version.source,
      file: version.file,
      lang,
      contentLang: version.contentLang,
      kind: version.kind,
      stale: version.stale,
      generated: isGenerated(entry),
      status: status(entry),
      groupKey,
    };
  };

  const groups: DocGroup[] = groupSpecs.map((spec) => ({
    key: spec.key,
    items: spec.items.flatMap(([id, label]) => {
      const entry = byId.get(id);
      return entry ? [item(entry, label, spec.key)] : [];
    }),
  }));

  // Die ADRs nach Nummer, die Vorlage ans Ende.
  const decisions = groups.find((group) => group.key === 'decisions')!;
  const adrs = entries
    .filter((entry) => entry.id.startsWith('adr/') && entry.id !== 'adr/readme')
    .sort((a, b) => {
      const ta = a.id.includes('template') ? 1 : 0;
      const tb = b.id.includes('template') ? 1 : 0;
      return ta - tb || a.id.localeCompare(b.id);
    });
  decisions.items.push(...adrs.map((entry) => item(entry, null, 'decisions')));

  const rest = entries.filter((entry) => !used.has(entry.id)).sort((a, b) => a.id.localeCompare(b.id));
  if (rest.length) groups.push({ key: 'more', items: rest.map((entry) => item(entry, null, 'more')) });

  return groups.filter((group) => group.items.length);
}

/** Vorige und nächste Seite innerhalb derselben Gruppe. */
export function neighbours(groups: DocGroup[], id: string): { prev?: DocItem; next?: DocItem } {
  for (const group of groups) {
    const index = group.items.findIndex((item) => item.id === id);
    if (index === -1) continue;
    return { prev: group.items[index - 1], next: group.items[index + 1] };
  }
  return {};
}

export function findItem(groups: DocGroup[], id: string): DocItem | undefined {
  return groups.flatMap((group) => group.items).find((item) => item.id === id);
}

/** Die Wege einer Seite in beiden Sprachen, für die Sprachwahl, hreflang und die Sitemap. */
export function docAlternates(id: string): { lang: DocLang; href: string }[] {
  return (docLangs as DocLang[]).map((lang) => ({ lang, href: docHref(id, lang) }));
}

/** Der Parameter `slug` einer Seite, gleich in beiden Sprachen (`guides/install`, leer für die Startseite). */
export const docSlug = (id: string) => docHref(id).replace(/^\/docs\/|\/$/g, '');
