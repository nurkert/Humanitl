// install.sh und uninstall.sh aus dem Wurzelverzeichnis des Repositorys (HUM-528, HUM-550), für
// die Dateien unter /install und /uninstall.
// Nur aus astro.config.mjs zu nutzen, das ungebündelt läuft: In einer Seite zeigte repoRoot nach
// dem Bündeln in dist/. Die Leseseite liest dieselbe Datei deshalb über Vite (`?raw`).
import { readFileSync } from 'node:fs';
import path from 'node:path';

import { repoRoot } from './docs-catalog.mjs';

export const installScriptPath = path.join(repoRoot, 'install.sh');

/** Der Inhalt als Bytes, genau so, wie er unter /install ausgeliefert wird. */
export const installScriptBytes = () => readFileSync(installScriptPath);

export const uninstallScriptPath = path.join(repoRoot, 'uninstall.sh');

/** Der Inhalt als Bytes, genau so, wie er unter /uninstall ausgeliefert wird. */
export const uninstallScriptBytes = () => readFileSync(uninstallScriptPath);
