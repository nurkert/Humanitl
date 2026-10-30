// Markdown-Baum der Dateien aus docs/ für die Website herrichten, ohne den Inhalt zu
// ändern (HUM-336):
// - relative Verweise auf andere Dokumente zeigen auf deren Seite unter /docs/, mit Anker;
//   Verweise auf andere Dateien des Repositorys zeigen auf GitHub (blob, bei Ordnern tree);
// - relative Bildpfade bekommen ein `./`, damit die Asset-Pipeline sie als lokale Datei
//   auflöst und gehasht ausliefert;
// - in einer Übersetzung unter docs/translations/<lang>/ (HUM-346) gelten alle relativen
//   Ziele relativ zum Original, wie im Leitfaden der Übersetzer festgelegt; Bildpfade werden
//   auf den Ordner der Übersetzung umgerechnet, damit dieselbe Bilddatei geladen wird;
// - Abschnitte zwischen `<!-- lang: de -->` und `<!-- /lang -->` werden ein
//   `<div class="lang-block" data-lang="de">`. Bewusst ohne `lang`: die Marker umschließen
//   englische Prosa und Tabellen, die deutsche Strings nur zitieren; ein `lang="de"` ließe
//   Screenreader ganze englische Sätze mit deutscher Stimme lesen;
// - alle übrigen HTML-Kommentare fallen weg, wie auf GitHub.
import { statSync } from 'node:fs';
import path from 'node:path';

import { docHref, docId, docsRoot, githubBlob, githubTree, repoRoot, translationInfo } from './docs-catalog.mjs';

const commentOnly = /^\s*<!--[\s\S]*?-->\s*$/;
const langOpen = /^\s*<!--\s*lang:\s*([a-z]{2}(?:-[A-Za-z]{2})?)\s*-->\s*$/;
const langClose = /^\s*<!--\s*\/lang\s*-->\s*$/;

/** Ein Ziel umschreiben; alles, was nicht relativ ist, bleibt, wie es ist. */
export function rewriteDocLink(url, filePath) {
  if (!url || /^(?:[a-z][a-z0-9+.-]*:|#|\/)/i.test(url)) return url;
  // Pfad, Abfrage und Anker getrennt: `../UX.md?x=1#a` ist ein Verweis auf UX.md.
  const hashAt = url.indexOf('#');
  const beforeHash = hashAt === -1 ? url : url.slice(0, hashAt);
  const queryAt = beforeHash.indexOf('?');
  const target = queryAt === -1 ? beforeHash : beforeHash.slice(0, queryAt);
  const anchor = (queryAt === -1 ? '' : beforeHash.slice(queryAt)) + (hashAt === -1 ? '' : url.slice(hashAt));
  if (!target) return url;
  const absolute = path.resolve(path.dirname(filePath), decodeURIComponent(target));
  const inDocs = absolute === docsRoot || absolute.startsWith(docsRoot + path.sep);
  if (inDocs && /\.md$/i.test(absolute)) {
    return docHref(docId(path.relative(docsRoot, absolute))) + anchor;
  }
  const relative = path.relative(repoRoot, absolute).split(path.sep).join('/');
  if (relative.startsWith('..') || path.isAbsolute(relative)) return url;
  // Ordner oder Datei entscheidet das Repository selbst (LICENSE hat keine Endung, ist aber
  // eine Datei); gibt es den Pfad nicht, gilt ein Schrägstrich am Ende als Ordner.
  let isDir = target.endsWith('/');
  try {
    isDir = statSync(absolute).isDirectory();
  } catch {
    // nicht vorhanden: die Prüfung der Links meldet es
  }
  return (isDir ? githubTree : githubBlob) + relative + (isDir && target.endsWith('/') ? '/' : '') + anchor;
}

/**
 * Ein relativer Bildpfad, gelesen relativ zu `linkBase` (dem Original), als Pfad relativ zur
 * Datei `filePath`, immer mit `./` oder `../` davor, damit die Asset-Pipeline ihn als lokale
 * Datei nimmt. Absolute Wege und Adressen bleiben.
 */
export function rewriteImage(url, filePath, linkBase = filePath) {
  if (!url || /^(?:[a-z][a-z0-9+.-]*:|\/|#)/i.test(url)) return url;
  if (linkBase === filePath) return url.startsWith('.') ? url : `./${url}`;
  const target = path.resolve(path.dirname(linkBase), decodeURI(url));
  const relative = path.relative(path.dirname(filePath), target).split(path.sep).join('/');
  return relative.startsWith('.') ? relative : `./${relative}`;
}

function walk(node, visit) {
  visit(node);
  if (node.children) for (const child of node.children) walk(child, visit);
}

/** Kommentare entfernen und lang-Abschnitte einwickeln, in jeder Ebene des Baums. */
function restructure(parent) {
  if (!parent.children) return;
  const out = [];
  let open = null;
  for (const child of parent.children) {
    if (child.type === 'html' && langOpen.test(child.value)) {
      const lang = langOpen.exec(child.value)[1];
      open = {
        type: 'docsLang',
        data: { hName: 'div', hProperties: { dataLang: lang, className: ['lang-block'] } },
        children: [],
      };
      out.push(open);
      continue;
    }
    if (child.type === 'html' && langClose.test(child.value)) {
      open = null;
      continue;
    }
    if (child.type === 'html' && commentOnly.test(child.value)) continue;
    restructure(child);
    (open ? open.children : out).push(child);
  }
  parent.children = out;
}

export default function remarkDocs() {
  return (tree, file) => {
    const filePath = file.path ?? file.history?.[0];
    if (!filePath || !filePath.startsWith(docsRoot + path.sep)) return;
    const translation = translationInfo(filePath);
    const linkBase = translation ? translation.sourcePath : filePath;
    walk(tree, (node) => {
      if (node.type === 'link' || node.type === 'definition') {
        node.url = rewriteDocLink(node.url, linkBase);
      } else if (node.type === 'image') {
        node.url = rewriteImage(node.url, filePath, linkBase);
      }
    });
    restructure(tree);
  };
}
