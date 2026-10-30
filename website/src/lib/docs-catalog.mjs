// Was für die Docs-Seiten fest steht und sowohl die Markdown-Plugins (astro.config.mjs)
// als auch die Seiten brauchen: Wurzeln, Wege, Sprache und die Repo-Adresse (HUM-336).
// Absichtlich reines JavaScript, damit die Astro-Konfiguration es ohne Übersetzung lädt.
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const repoRoot = fileURLToPath(new URL('../../../', import.meta.url));
export const docsRoot = path.join(repoRoot, 'docs');

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

/** Der Weg einer Seite. README.md eines Ordners wird dessen Startseite (`/docs/`, `/docs/adr/`). */
export function docHref(id) {
  const route = id === 'readme' ? '' : id.replace(/(^|\/)readme$/, '');
  return `/docs/${route ? `${route}/` : ''}`;
}
