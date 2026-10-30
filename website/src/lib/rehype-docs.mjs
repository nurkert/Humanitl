// HTML-Baum der Dateien aus docs/ in die Form der Website bringen (HUM-336). Der Text
// bleibt Zeichen für Zeichen gleich, nur das Markup wächst:
// - Überschriften H2 bis H4 bekommen einen Anker-Link, die IDs bildet github-slugger wie
//   auf GitHub (dieselbe Funktion, die Astro danach für das Inhaltsverzeichnis nutzt);
// - die Metazeilen unter der H1 („Applies to", bei ADRs „Status" und „Date") werden Chips;
// - GitHub-Hinweisboxen (`> [!NOTE]` usw.) werden Boxen mit Titel;
// - Codeblöcke werden eine `figure` mit Sprachleiste, Tabellen ein scrollbarer Bereich;
// - ein Absatz nur aus Bildern wird eine Bildreihe;
// - rohes HTML wird wie auf GitHub auf eine Liste erlaubter Elemente beschränkt;
// - eine Übersetzung (HUM-346) bekommt die IDs der Überschriften ihres Originals, Stelle für
//   Stelle, damit Anker und Sprachwechsel in beiden Sprachen auf denselben Abschnitt zeigen.
//   Weicht die Gliederung ab, behält sie ihre eigenen IDs, und der Bau meldet es, ohne zu
//   scheitern; jede ID des Originals bleibt dann als leerer Anker erreichbar;
// - die festen Wörter der Website (Chips, Hinweisboxen, Beschriftungen) stehen in der Sprache
//   des Dokuments.
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { createMarkdownProcessor, rehypeHeadingIds } from '@astrojs/markdown-remark';
import { raw } from 'hast-util-raw';

import { docId, docsRoot, germanDocs, translationInfo } from './docs-catalog.mjs';
import remarkDocs from './remark-docs.mjs';

const el = (tagName, properties = {}, children = []) => ({ type: 'element', tagName, properties, children });
const text = (value) => ({ type: 'text', value });

function toText(node) {
  if (node.type === 'text') return node.value;
  return node.children ? node.children.map(toText).join('') : '';
}

const isWhitespace = (node) => node.type === 'text' && !node.value.trim();

const languageNames = {
  sh: 'Shell',
  bash: 'Shell',
  shell: 'Shell',
  console: 'Shell',
  yaml: 'YAML',
  yml: 'YAML',
  rust: 'Rust',
  json: 'JSON',
  toml: 'TOML',
  dart: 'Dart',
  ts: 'TypeScript',
  js: 'JavaScript',
  proto: 'Protobuf',
  diff: 'Diff',
};

// Die festen Wörter je Sprache des Dokuments. Bewusst hier und nicht in src/i18n: die
// Plugins laufen auch in den Tests unter Node, ohne TypeScript.
const words = {
  en: {
    alerts: { NOTE: 'Note', TIP: 'Tip', IMPORTANT: 'Important', WARNING: 'Warning', CAUTION: 'Caution' },
    meta: { 'Applies to:': 'Applies to:', 'Status:': 'Status:', 'Date:': 'Date:' },
    status: {},
    generated: 'Generated',
    generatedTitle: 'Generated from the code by cargo xtask docs',
    section: (label) => `Section titled “${label}”`,
    table: 'Table',
  },
  de: {
    alerts: { NOTE: 'Hinweis', TIP: 'Tipp', IMPORTANT: 'Wichtig', WARNING: 'Warnung', CAUTION: 'Vorsicht' },
    meta: { 'Applies to:': 'Gilt für:', 'Status:': 'Status:', 'Date:': 'Datum:' },
    status: { Accepted: 'Angenommen', Superseded: 'Ersetzt', Deprecated: 'Veraltet', Proposed: 'Vorgeschlagen' },
    generated: 'Erzeugt',
    generatedTitle: 'Mit cargo xtask docs aus dem Code erzeugt',
    section: (label) => `Abschnitt „${label}“`,
    table: 'Tabelle',
  },
};

/** Statuswerte übersetzen, Wort für Wort, damit „Accepted | Superseded by ADR-XXXX" lesbar bleibt. */
const statusText = (value, w) => value.replace(/\b(Accepted|Superseded|Deprecated|Proposed)\b/g, (word) => w.status[word] ?? word);

// Octicons-ähnliche Umrisse, 16er Raster, nur als Schmuck (aria-hidden).
const alertIcons = {
  NOTE: 'M8 1.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM8 7v4.5M8 4.75v.5',
  TIP: 'M8 1.5a4.5 4.5 0 0 0-2.5 8.24V11.5h5V9.74A4.5 4.5 0 0 0 8 1.5ZM6 14h4',
  IMPORTANT: 'M2 2.5h12v8.5H8.5L5 14v-3H2V2.5ZM8 5v3M8 9.75v.5',
  WARNING: 'M8 1.75 1.25 13.75h13.5L8 1.75ZM8 6.5v3.25M8 11.5v.5',
  CAUTION: 'M5.25 1.5h5.5l3.75 3.75v5.5l-3.75 3.75h-5.5L1.5 10.75v-5.5L5.25 1.5ZM8 4.75V8.5M8 10.75v.5',
};

function icon(type) {
  return el(
    'svg',
    { viewBox: '0 0 16 16', width: 14, height: 14, ariaHidden: 'true', focusable: 'false', className: ['alert__icon'] },
    [
      el('path', {
        d: alertIcons[type],
        fill: 'none',
        stroke: 'currentColor',
        strokeWidth: 1.5,
        strokeLinecap: 'round',
        strokeLinejoin: 'round',
      }),
    ],
  );
}

const metaLine = /^(Applies to|Status|Date):\s/;

/** Ein Absatz aus Metazeilen: nur Text, jede Zeile „Schlüssel: Wert". */
function isMetaParagraph(node) {
  if (node?.type !== 'element' || node.tagName !== 'p') return false;
  if (!node.children.every((child) => child.type === 'text')) return false;
  const lines = toText(node).split('\n').filter(Boolean);
  return lines.length > 0 && lines.every((line) => metaLine.test(line));
}

function chip(key, value, extra = {}) {
  return el('span', { className: ['chip'], ...extra }, [el('span', { className: ['chip__k'] }, [text(key)]), text(value)]);
}

function metaChips(paragraphs, flags, w) {
  const chips = [];
  for (const paragraph of paragraphs) {
    for (const line of toText(paragraph).split('\n').filter(Boolean)) {
      const colon = line.indexOf(':');
      const key = line.slice(0, colon + 1);
      const value = line.slice(colon + 1);
      const extra = key === 'Status:' ? { dataStatus: value.trim().split(/\s/)[0].toLowerCase() } : {};
      chips.push(chip(w.meta[key] ?? key, key === 'Status:' ? statusText(value, w) : value, extra));
    }
  }
  if (flags.generated) {
    chips.push(el('span', { className: ['chip', 'chip--flag'], title: w.generatedTitle }, [text(w.generated)]));
  }
  return el('div', { className: ['doc-meta'] }, chips);
}

/** `> [!NOTE]` am Anfang eines Blockzitats erkennen und in eine Box umbauen. */
function toAlert(node, w) {
  const first = node.children.find((child) => child.type === 'element');
  if (!first || first.tagName !== 'p') return false;
  const lead = first.children[0];
  if (lead?.type !== 'text') return false;
  const match = /^\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\][ \t]*(?:\r?\n|$)/.exec(lead.value);
  if (!match) return false;
  const type = match[1];
  lead.value = lead.value.slice(match[0].length);
  if (!lead.value) first.children.shift();
  if (first.children[0]?.type === 'element' && first.children[0].tagName === 'br') first.children.shift();
  if (!first.children.length || first.children.every(isWhitespace)) {
    node.children.splice(node.children.indexOf(first), 1);
  }
  node.tagName = 'div';
  node.properties = { className: ['alert', `alert--${type.toLowerCase()}`], role: 'note' };
  node.children.unshift(el('p', { className: ['alert__title'] }, [icon(type), text(w.alerts[type])]));
  return true;
}

function languageOf(pre) {
  const props = pre.properties ?? {};
  if (typeof props.dataLanguage === 'string') return props.dataLanguage;
  const code = pre.children.find((child) => child.type === 'element' && child.tagName === 'code');
  const classes = [...(props.className ?? []), ...(code?.properties?.className ?? [])];
  for (const cls of classes) {
    const match = /^language-(\S+)$/.exec(String(cls));
    if (match) return match[1];
  }
  return 'plaintext';
}

function codeFigure(pre) {
  const lang = languageOf(pre);
  const name = languageNames[lang];
  const children = [];
  if (name) {
    children.push(el('figcaption', { className: ['code__bar'] }, [el('span', { className: ['code__lang'] }, [text(name)])]));
  }
  children.push(pre);
  return el('figure', { className: ['code', name ? 'code--bar' : 'code--bare'], dataLang: lang }, children);
}

function anchorFor(heading, w) {
  const id = heading.properties.id;
  const label = toText(heading).trim();
  return el('a', { className: ['anchor'], href: `#${id}` }, [
    el('span', { ariaHidden: 'true' }, [text('#')]),
    el('span', { className: ['visually-hidden'] }, [text(w.section(label))]),
  ]);
}

const tableParts = new Set(['table', 'thead', 'tbody', 'tfoot', 'tr']);

function stripTableWhitespace(node) {
  if (node.type !== 'element' || !tableParts.has(node.tagName)) return;
  node.children = node.children.filter((child) => !isWhitespace(child));
  for (const child of node.children) stripTableWhitespace(child);
}

// Elemente, die GitHub in Markdown durchlässt (angelehnt an das GitHub-Schema von
// hast-util-sanitize), dazu `section` für Fußnoten. Alles andere wird ausgepackt: die Kinder
// bleiben, das Element fällt weg. So schließt etwa ein `<dir>` am Zeilenanfang, das CommonMark
// als HTML-Block liest, nicht mehr den Rest des Artikels ein (docs/cli.md).
export const allowedTags = new Set([
  'a', 'abbr', 'b', 'bdo', 'blockquote', 'br', 'caption', 'cite', 'code', 'col', 'colgroup', 'dd',
  'del', 'details', 'dfn', 'div', 'dl', 'dt', 'em', 'figcaption', 'figure', 'h1', 'h2', 'h3', 'h4',
  'h5', 'h6', 'hr', 'i', 'img', 'input', 'ins', 'kbd', 'li', 'mark', 'ol', 'p', 'picture', 'pre',
  'q', 'rp', 'rt', 'ruby', 's', 'samp', 'section', 'small', 'source', 'span', 'strike', 'strong',
  'sub', 'summary', 'sup', 'table', 'tbody', 'td', 'tfoot', 'th', 'thead', 'time', 'tr', 'tt', 'u',
  'ul', 'var', 'wbr',
]);
// Diese Elemente verschwinden samt Inhalt, weil ihr Inhalt kein Lesetext ist.
const droppedTags = new Set(['script', 'style', 'iframe', 'object', 'embed', 'template', 'noscript', 'link', 'meta', 'form', 'textarea', 'select', 'button']);

// Attribute, die jedes erlaubte Element tragen darf, und die je Element dazu (Namen in
// hast-Schreibweise). Alles andere fällt weg, auch `style`: über die CSP dürfen Inline-Stile
// laufen, und ein Dokument könnte sonst die ganze Seite überdecken. Nur innerhalb von
// <pre> bleibt `style`, weil die Hervorhebung von Shiki dort ihre Farben setzt.
const globalAttributes = new Set(['id', 'className', 'title', 'lang', 'dir', 'dataLang', 'role', 'ariaHidden', 'ariaLabel']);
const elementAttributes = {
  a: new Set(['href', 'name', 'rel']),
  img: new Set(['src', 'alt', 'width', 'height', 'loading', 'decoding']),
  source: new Set(['srcSet', 'media', 'type', 'width', 'height']),
  td: new Set(['colSpan', 'rowSpan', 'align']),
  th: new Set(['colSpan', 'rowSpan', 'align', 'scope']),
  col: new Set(['span']),
  colgroup: new Set(['span']),
  ol: new Set(['start', 'reversed', 'type']),
  li: new Set(['value']),
  details: new Set(['open']),
  input: new Set(['type', 'checked', 'disabled']),
  time: new Set(['dateTime']),
  q: new Set(['cite']),
  blockquote: new Set(['cite']),
  ins: new Set(['cite', 'dateTime']),
  del: new Set(['cite', 'dateTime']),
  code: new Set(['dataLanguage']),
  pre: new Set(['dataLanguage', 'tabIndex']),
};
const urlAttributes = new Set(['href', 'src', 'cite', 'srcSet']);

/** Ein Ziel ist sicher, wenn es relativ ist oder http, https oder mailto nennt. */
export function isSafeUrl(value) {
  // Steuerzeichen und Leerraum entfernen, wie Browser es vor dem Auswerten tun
  // (`java\tscript:` ist sonst ein javascript:-Ziel).
  const url = String(value).replace(/[\u0000-\u0020\u007f-\u009f]/g, '').toLowerCase();
  const scheme = /^([a-z][a-z0-9+.-]*):/.exec(url);
  return !scheme || ['http', 'https', 'mailto'].includes(scheme[1]);
}

/** Rohes HTML auf erlaubte Elemente und Attribute beschränken, unsichere Ziele entfernen. */
function sanitize(parent, inPre = false) {
  if (!parent.children) return;
  const out = [];
  for (const child of parent.children) {
    if (child.type !== 'element') {
      if (child.type !== 'raw' && child.type !== 'comment') out.push(child);
      continue;
    }
    if (droppedTags.has(child.tagName)) continue;
    const childInPre = inPre || child.tagName === 'pre';
    sanitize(child, childInPre);
    if (!allowedTags.has(child.tagName)) {
      out.push(...child.children);
      continue;
    }
    const allowed = elementAttributes[child.tagName];
    for (const key of Object.keys(child.properties ?? {})) {
      const value = child.properties[key];
      const keep =
        globalAttributes.has(key) || allowed?.has(key) || (childInPre && key === 'style') || /^data[A-Z]/.test(key) && childInPre;
      if (!keep || (urlAttributes.has(key) && !isSafeUrl(value))) delete child.properties[key];
    }
    out.push(child);
  }
  parent.children = out;
}

/** Alle Kinder eines Knotens umbauen; `state` merkt sich die letzte Überschrift für Tabellen. */
function transform(parent, state) {
  const out = [];
  for (const child of parent.children ?? []) {
    if (child.type !== 'element') {
      out.push(child);
      continue;
    }
    const tag = child.tagName;
    if (/^h[2-4]$/.test(tag) && child.properties?.id) {
      state.heading = toText(child).trim();
      out.push(el('div', { className: ['hwrap', `hwrap--${tag}`] }, [child, anchorFor(child, state.words)]));
      continue;
    }
    if (tag === 'pre') {
      out.push(codeFigure(child));
      continue;
    }
    if (tag === 'table') {
      // Leerraum zwischen Tabellenzeilen wirft der HTML-Parser sonst vor die Tabelle.
      stripTableWhitespace(child);
      transform(child, state);
      out.push(
        el(
          'div',
          {
            className: ['table-wrap'],
            role: 'region',
            tabIndex: 0,
            ariaLabel: state.heading ? `${state.words.table}: ${state.heading}` : state.words.table,
          },
          [child],
        ),
      );
      continue;
    }
    if (tag === 'blockquote') toAlert(child, state.words);
    if (tag === 'p') {
      const elements = child.children.filter((node) => !isWhitespace(node));
      if (elements.length > 0 && elements.every((node) => node.type === 'element' && node.tagName === 'img')) {
        child.properties = { ...child.properties, className: ['img-row'] };
      }
    }
    transform(child, state);
    out.push(child);
  }
  parent.children = out;
}

/** Alle Überschriften H1 bis H6 in Dokumentreihenfolge, wie Astro sie für das Verzeichnis sammelt. */
export function collectHeadings(tree) {
  const found = [];
  (function visit(node) {
    if (node.type === 'element' && /^h[1-6]$/.test(node.tagName)) found.push(node);
    if (node.children) node.children.forEach(visit);
  })(tree);
  return found;
}

/**
 * Den Überschriften einer Übersetzung die IDs des Originals geben, Stelle für Stelle. Nur
 * wenn Anzahl und Ebenen übereinstimmen; sonst bleibt alles, wie es ist, und das Ergebnis
 * nennt den Grund.
 */
export function applySourceIds(headings, sourceHeadings) {
  const own = headings.map((node) => Number(node.tagName.slice(1)));
  const theirs = sourceHeadings.map((heading) => heading.depth);
  if (own.length !== theirs.length) {
    return { ok: false, reason: `${own.length} headings, the original has ${theirs.length}` };
  }
  const at = own.findIndex((depth, index) => depth !== theirs[index]);
  if (at !== -1) {
    return { ok: false, reason: `heading ${at + 1} is h${own[at]}, in the original h${theirs[at]}` };
  }
  headings.forEach((node, index) => {
    node.properties.id = sourceHeadings[index].slug;
  });
  return { ok: true };
}

/** Ziffernfolgen einer Überschrift („3.3", „0019"); sie bleiben beim Übersetzen gleich. */
const digitsOf = (value) => (String(value).match(/\d+(?:\.\d+)*/g) ?? []).join(' ');

/**
 * Überschriften einer Übersetzung und ihres Originals einander zuordnen, wenn die Gliederung
 * abweicht: längste gemeinsame Teilfolge über die Ebenen. Ein Paar braucht dieselbe Ebene; tragen
 * beide Ziffern („3.3"), müssen sie gleich sein, und gleiche Ziffern wiegen doppelt. Liefert je
 * Überschrift des Originals den Index der Überschrift in der Übersetzung oder -1.
 */
export function alignHeadings(own, theirs) {
  const n = own.length;
  const m = theirs.length;
  const weight = (i, j) => {
    if (own[i].depth !== theirs[j].depth) return 0;
    const a = own[i].digits;
    const b = theirs[j].digits;
    if (a && b) return a === b ? 2 : 0;
    return 1;
  };
  const score = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      const w = weight(i, j);
      score[i][j] = Math.max(score[i + 1][j], score[i][j + 1], w ? score[i + 1][j + 1] + w : 0);
    }
  }
  const match = new Array(m).fill(-1);
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    const w = weight(i, j);
    if (w && score[i][j] === score[i + 1][j + 1] + w) {
      match[j] = i;
      i += 1;
      j += 1;
    } else if (score[i + 1][j] >= score[i][j + 1]) i += 1;
    else j += 1;
  }
  return match;
}

/**
 * Rückfall bei abweichender Gliederung: Die Überschriften behalten ihre eigenen IDs, aber jede
 * ID des Originals bleibt erreichbar. Wo sie in der Übersetzung fehlt, steht ein leerer Anker
 * `<a id>` vor der zugeordneten Überschrift, ohne Zuordnung vor der nächsten zugeordneten und
 * hinter dem letzten Abschnitt am Ende. So zerbricht kein Verweis aus einer anderen Seite,
 * wenn das Original eine Überschrift gewinnt oder verliert. Liefert die Zahl der Anker.
 */
export function keepSourceAnchors(tree, headings, sourceHeadings) {
  const parents = new Map();
  const ids = new Set();
  (function visit(node) {
    if (node.properties?.id) ids.add(String(node.properties.id));
    for (const child of node.children ?? []) {
      parents.set(child, node);
      visit(child);
    }
  })(tree);
  const match = alignHeadings(
    headings.map((node) => ({ depth: Number(node.tagName.slice(1)), digits: digitsOf(toText(node)) })),
    sourceHeadings.map((heading) => ({ depth: heading.depth, digits: digitsOf(heading.text) })),
  );
  let added = 0;
  sourceHeadings.forEach((heading, index) => {
    if (!heading.slug || ids.has(heading.slug)) return;
    ids.add(heading.slug);
    const next = match.slice(index).find((own) => own !== -1);
    const anchor = el('a', { id: heading.slug, className: ['source-anchor'], ariaHidden: 'true' });
    const before = next === undefined ? undefined : headings[next];
    const parent = before ? parents.get(before) : tree;
    if (before && parent) parent.children.splice(parent.children.indexOf(before), 0, anchor);
    else tree.children.push(anchor);
    added += 1;
  });
  return added;
}

let sourceProcessor;
const sourceCache = new Map();

/**
 * Die Überschriften eines Originals mit den IDs, die seine eigene Seite trägt: dasselbe
 * Markdown durch dieselben Plugins, ohne Hervorhebung, weil nur die Überschriften zählen.
 */
async function headingsOf(sourcePath, readSource) {
  const content = await readSource(sourcePath);
  const key = `${sourcePath}\u0000${content}`;
  if (!sourceCache.has(key)) {
    sourceProcessor ??= createMarkdownProcessor({
      smartypants: false,
      remarkPlugins: [remarkDocs],
      rehypePlugins: [rehypeDocs],
      syntaxHighlight: false,
    });
    sourceCache.set(
      key,
      (await sourceProcessor)
        .render(content, { fileURL: pathToFileURL(sourcePath) })
        .then(({ metadata }) => metadata.headings),
    );
  }
  return sourceCache.get(key);
}

/**
 * `readSource` liest ein Original (Vorgabe: von der Platte); die Tests geben eine eigene
 * Funktion mit, damit sie ohne echte Dateien unter docs/ auskommen.
 */
export default function rehypeDocs(options = {}) {
  const readSource = options.readSource ?? ((file) => readFile(file, 'utf8'));
  const assignIds = rehypeHeadingIds();
  return async (tree, file) => {
    const filePath = file.path ?? file.history?.[0];
    if (!filePath || !filePath.startsWith(docsRoot + path.sep)) return;
    // Rohes HTML liegt hier noch als Zeichenkette vor (Astros rehype-raw läuft erst nach den
    // eigenen Plugins). Erst parsen, dann beschränken; sonst schlösse der Browser ein offenes
    // Element wie `<dir>` erst vor `</article>`.
    // Leerraum zwischen Tabellenzeilen vorher entfernen, sonst schiebt der HTML-Parser ihn
    // vor die Tabelle.
    (function stripTables(node) {
      if (node.type === 'element' && node.tagName === 'table') stripTableWhitespace(node);
      else if (node.children) node.children.forEach(stripTables);
    })(tree);
    const parsed = raw(tree, { file });
    if (parsed !== tree) Object.assign(tree, parsed);
    sanitize(tree);
    // Die IDs schon hier vergeben, damit die Anker sie kennen. Astros eigener Lauf danach
    // findet sie vor, übernimmt sie unverändert und baut daraus die Liste für das Inhaltsverzeichnis.
    assignIds(tree, file);

    const translation = translationInfo(filePath);
    if (translation) {
      let result;
      let sourceHeadings;
      try {
        sourceHeadings = await headingsOf(translation.sourcePath, readSource);
        result = applySourceIds(collectHeadings(tree), sourceHeadings);
      } catch (error) {
        result = { ok: false, reason: `original not readable (${error.code ?? error.message})` };
      }
      if (!result.ok) {
        const rel = path.relative(docsRoot, filePath).split(path.sep).join('/');
        const anchors = sourceHeadings ? keepSourceAnchors(tree, collectHeadings(tree), sourceHeadings) : 0;
        const kept = anchors ? `, ${anchors} anchor(s) of the original added` : '';
        console.warn(`[docs] ${rel}: ${result.reason}; the page keeps its own heading ids${kept}.`);
        // Für die Seite (DocPage markiert den Artikel) und damit check-links tote Anker in
        // diese Seite nur als Warnung meldet.
        file.data.astro ??= {};
        file.data.astro.frontmatter = { ...(file.data.astro.frontmatter ?? {}), headingFallback: true };
      }
    }

    const id = docId(path.relative(docsRoot, filePath));
    const lang = translation ? translation.lang : germanDocs.has(id) ? 'de' : 'en';
    const w = words[lang] ?? words.en;
    const source = String(file.value ?? '');
    const flags = {
      generated: /^<!--\s*(?:Erzeugt|Generated)\b/m.test(source.split('\n').slice(0, 12).join('\n')),
    };

    // Metazeilen direkt unter der H1.
    const children = tree.children;
    const h1 = children.findIndex((node) => node.type === 'element' && node.tagName === 'h1');
    if (h1 !== -1) {
      const meta = [];
      let index = h1 + 1;
      let last = h1;
      while (index < children.length) {
        const node = children[index];
        if (isWhitespace(node)) {
          index += 1;
          continue;
        }
        if (!isMetaParagraph(node)) break;
        meta.push(node);
        last = index;
        index += 1;
      }
      if (meta.length || flags.generated) {
        children.splice(h1 + 1, last - h1, text('\n'), metaChips(meta, flags, w));
      }
    }

    transform(tree, { heading: '', words: w });
  };
}
