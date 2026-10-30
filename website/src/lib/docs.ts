// Ordnung und Beschriftung der Docs-Seiten (HUM-336). Die Gruppen folgen docs/README.md;
// Dateien, die hier fehlen, landen in „More", damit keine Datei ohne Seite bleibt.
import type { CollectionEntry } from 'astro:content';

import { docHref, germanDocs, githubBlob, repoUrl } from './docs-catalog.mjs';

export { docHref, repoUrl };

export type DocEntry = CollectionEntry<'docs'>;

interface GroupSpec {
  key: string;
  title: string;
  /** Feste Reihenfolge mit kurzem Label; `null` nimmt den Titel der Datei. */
  items: [id: string, label: string | null][];
}

const groupSpecs: GroupSpec[] = [
  { key: 'start', title: 'Start', items: [['readme', 'Overview']] },
  {
    key: 'using',
    title: 'Using',
    items: [
      ['guides/install', 'Install'],
      ['guides/first-session', 'First session'],
      ['guides/agent-profiles', 'Agent profiles'],
      ['profiles', 'Profiles (stub)'],
    ],
  },
  {
    key: 'reference',
    title: 'Reference',
    items: [
      ['reference/rules', 'Rules'],
      ['reference/cli', 'CLI reference'],
      ['config', 'Configuration'],
      ['diagnostics', 'Diagnostic codes'],
      ['reference/limits', 'Resource limits'],
      ['reference/parity', 'RPC parity'],
      ['cli', 'CLI walkthrough'],
      ['protocol', 'Protocol'],
      ['glossary', 'Glossary'],
    ],
  },
  {
    key: 'security',
    title: 'Security',
    items: [
      ['security', 'Security'],
      ['threat-model', 'Threat model'],
    ],
  },
  {
    key: 'building',
    title: 'Building',
    items: [
      ['architecture', 'Architecture'],
      ['design', 'Design system'],
      ['ux', 'UX guidelines'],
      ['install', 'Maintainer notes'],
    ],
  },
  { key: 'decisions', title: 'Decisions', items: [['adr/readme', 'All decisions']] },
];

export interface DocItem {
  id: string;
  href: string;
  label: string;
  /** Nummer einer ADR („0001"), sonst leer. */
  number: string;
  title: string;
  summary: string;
  source: string;
  german: boolean;
  generated: boolean;
  status: string;
  group: string;
  groupKey: string;
}

export interface DocGroup {
  key: string;
  title: string;
  items: DocItem[];
}

const stripMarks = (value: string) => value.replace(/[*_`]/g, '').trim();

/** Die erste H1 ist der Titel, wie auf GitHub. */
export function docTitle(entry: DocEntry): string {
  const match = entry.body?.match(/^#\s+(.+)$/m);
  return match ? match[1].trim() : entry.id;
}

export function plainTitle(entry: DocEntry): string {
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
export function docSummary(entry: DocEntry): string {
  const lines = (entry.body ?? '').split('\n');
  const purpose = lines.findIndex((line) => /^>\s*Purpose:/.test(line));
  if (purpose !== -1) {
    const block: string[] = [];
    for (let i = purpose; i < lines.length && lines[i].startsWith('>'); i += 1) block.push(lines[i].replace(/^>\s?/, ''));
    return clip(plain(block.join(' ').replace(/^Purpose:\s*/, '')));
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

/** Die Spalte „What it answers" aus docs/README.md, nachgeschlagen über das Linkziel. */
function readmeSummaries(readme: DocEntry | undefined): Map<string, string> {
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

export function sourcePath(entry: DocEntry): string {
  const file = entry.filePath?.replace(/\\/g, '/') ?? '';
  const inDocs = file.includes('docs/') ? file.slice(file.lastIndexOf('docs/') + 5) : `${entry.id}.md`;
  return `docs/${inDocs}`;
}

export const blobUrl = (source: string) => `${githubBlob}${source}`;
export const editUrl = (source: string) => `${repoUrl}/edit/main/${source}`;
export const historyUrl = (source: string) => `${repoUrl}/commits/main/${source}`;

export function isGenerated(entry: DocEntry): boolean {
  return /^<!--\s*(?:Erzeugt|Generated)\b/m.test((entry.body ?? '').split('\n').slice(0, 12).join('\n'));
}

export const isGerman = (entry: DocEntry) => germanDocs.has(entry.id);

function status(entry: DocEntry): string {
  return entry.body?.match(/^Status:\s*(.+)$/m)?.[1].trim() ?? '';
}

/** Alle Seiten in Gruppen, in der Reihenfolge der Seitenleiste. */
export function docGroups(entries: DocEntry[]): DocGroup[] {
  const byId = new Map(entries.map((entry) => [entry.id, entry]));
  const summaries = readmeSummaries(byId.get('readme'));
  const used = new Set<string>();

  const item = (entry: DocEntry, label: string | null, group: GroupSpec | { key: string; title: string }): DocItem => {
    used.add(entry.id);
    const title = plainTitle(entry);
    const adr = /^ADR-(\d{4}|NNNN)\s*·\s*(.+)$/.exec(title);
    let number = '';
    let shortLabel = label ?? title;
    if (!label && adr) {
      number = adr[1] === 'NNNN' ? '0000' : adr[1];
      shortLabel = adr[1] === 'NNNN' ? 'Template' : adr[2];
    }
    return {
      id: entry.id,
      href: docHref(entry.id),
      label: shortLabel,
      number,
      title,
      summary: summaries.get(entry.id) ?? docSummary(entry),
      source: sourcePath(entry),
      german: isGerman(entry),
      generated: isGenerated(entry),
      status: status(entry),
      group: group.title,
      groupKey: group.key,
    };
  };

  const groups: DocGroup[] = groupSpecs.map((spec) => ({
    key: spec.key,
    title: spec.title,
    items: spec.items.flatMap(([id, label]) => {
      const entry = byId.get(id);
      return entry ? [item(entry, label, spec)] : [];
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
  decisions.items.push(...adrs.map((entry) => item(entry, null, decisions)));

  const more = { key: 'more', title: 'More' };
  const rest = entries.filter((entry) => !used.has(entry.id)).sort((a, b) => a.id.localeCompare(b.id));
  if (rest.length) groups.push({ ...more, items: rest.map((entry) => item(entry, null, more)) });

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
