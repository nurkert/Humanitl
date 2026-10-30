// Was für die Docs-Seiten fest steht und sowohl die Markdown-Plugins (astro.config.mjs)
// als auch die Seiten brauchen: Wurzeln, Wege, Sprache und die Repo-Adresse (HUM-336).
// Absichtlich reines JavaScript, damit die Astro-Konfiguration es ohne Übersetzung lädt.
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));
export const docsRoot = path.join(repoRoot, 'docs');
/** Übersetzungen: docs/translations/<lang>/<Pfad wie im Original> (HUM-346). */
export const translationsRoot = path.join(docsRoot, 'translations');

export const repoUrl = 'https://github.com/nurkert/Humanitl';
export const branch = 'main';
export const githubBlob = `${repoUrl}/blob/${branch}/`;
export const githubTree = `${repoUrl}/tree/${branch}/`;

/** Die Dokumente, die auf Deutsch geschrieben sind; alle anderen sind Englisch. */
export const germanDocs = new Set(['config', 'diagnostics', 'reference/parity']);

/**
 * Die ID einer Datei relativ zu docs/, so wie der Glob-Loader sie bildet: Pfad ohne
 * Endung, in Kleinbuchstaben. `adr/0001-rust-hudsucker.md` wird `adr/0001-rust-hudsucker`.
 */
export function docId(relativePath) {
  return relativePath
    .replace(/\\/g, '/')
    .replace(/\.md$/i, '')
    .split('/')
    .map((segment) => segment.toLowerCase())
    .join('/');
}

/** Die Sprachen der Docs; Deutsch ist die Standardsprache der Website und steht an der Wurzel. */
export const docLangs = ['de', 'en'];

/** Startseite der Docs in einer Sprache: `/docs/` auf Deutsch, `/en/docs/` auf Englisch. */
export const docsHome = (lang = 'de') => (lang === 'de' ? '/docs/' : `/${lang}/docs/`);

/**
 * Der Weg einer Seite. README.md eines Ordners wird dessen Startseite (`/docs/`, `/docs/adr/`).
 * Ohne Sprache der deutsche Weg; die Markdown-Plugins schreiben Verweise immer so, und die
 * englische Seite setzt das Präfix danach mit `localizeDocLinks`.
 */
export function docHref(id, lang = 'de') {
  const route = id === 'readme' ? '' : id.replace(/(^|\/)readme$/, '');
  return `${docsHome(lang)}${route ? `${route}/` : ''}`;
}

/** Die Sprache, in der ein Dokument geschrieben ist. */
export const originalLang = (id) => (germanDocs.has(id) ? 'de' : 'en');

/**
 * Verweise des gerenderten Artikels auf die Docs-Seiten der Sprache `lang` umstellen. Die
 * Plugins erzeugen `/docs/…`; im englischen Baum wird daraus `/en/docs/…`. Text in Codeblöcken
 * ist maskiert (`href=&quot;`) und bleibt darum unberührt.
 */
export function localizeDocLinks(html, lang) {
  if (lang === 'de') return html;
  return html.replace(/(\shref=")\/docs\//g, `$1${docsHome(lang)}`);
}

/**
 * Ist `filePath` eine Übersetzung, deren Sprache und Original; sonst `null`. Beides folgt aus
 * dem Pfad der Datei (docs/translations/<lang>/<Pfad des Originals>), nie aus dem Front Matter:
 * so lässt kein Front Matter den Bau Dateien außerhalb von docs/ lesen, und eine Übersetzung,
 * deren Front Matter dem Pfad widerspricht, überspringt docs.ts ohnehin als ungültig.
 */
export function translationInfo(filePath) {
  if (!filePath || !filePath.startsWith(translationsRoot + path.sep)) return null;
  const parts = path.relative(translationsRoot, filePath).split(path.sep);
  const source = parts.slice(1).join('/');
  return { lang: parts[0], source, sourcePath: path.resolve(docsRoot, source) };
}
