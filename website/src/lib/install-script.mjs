// install.sh aus dem Wurzelverzeichnis des Repositorys (HUM-528), für die Datei unter /install.
// Nur aus astro.config.mjs zu nutzen, das ungebündelt läuft: In einer Seite zeigte repoRoot nach
// dem Bündeln in dist/. Die Leseseite liest dieselbe Datei deshalb über Vite (`?raw`).
import { readFileSync } from 'node:fs';
import path from 'node:path';

import { repoRoot } from './docs-catalog.mjs';

export const installScriptPath = path.join(repoRoot, 'install.sh');

/** Der Inhalt als Bytes, genau so, wie er unter /install ausgeliefert wird. */
export const installScriptBytes = () => readFileSync(installScriptPath);
