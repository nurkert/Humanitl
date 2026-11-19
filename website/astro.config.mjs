import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

import { unified } from '@astrojs/markdown-remark';
import { defineConfig } from 'astro/config';

import { repoRoot } from './src/lib/docs-catalog.mjs';
import rehypeDocs from './src/lib/rehype-docs.mjs';
import remarkDocs from './src/lib/remark-docs.mjs';
import { installScriptBytes } from './src/lib/install-script.mjs';

// install.sh unverändert unter https://humanitl.de/install (HUM-528), für
// `curl -fsSL https://humanitl.de/install | sh`. Eine Datei ohne Endung, die GitHub Pages als
// application/octet-stream ausliefert; Lesen im Browser geht über die Seite /installationsskript/.
// Kopiert werden die Bytes, nicht Text: nichts darf Zeilenenden oder Kodierung ändern.
const installScript = {
  name: 'humanitl-install-script',
  hooks: {
    'astro:build:done': ({ dir }) => {
      writeFileSync(fileURLToPath(new URL('install', dir)), installScriptBytes());
    },
  },
};

export default defineConfig({
  site: 'https://humanitl.de',
  trailingSlash: 'always',
  integrations: [installScript],
  build: {
    // CSS und JS liegen in Dateien, damit die Content-Security-Policy ohne Inline-Skripte auskommt.
    inlineStylesheets: 'never',
  },
  // Die Dokumentation aus docs/ (HUM-336). Typografische Ersetzungen bleiben aus, damit
  // `--flag`, Apostrophe und Anführungszeichen so erscheinen wie auf GitHub.
  markdown: {
    processor: unified({
      smartypants: false,
      remarkPlugins: [remarkDocs],
      rehypePlugins: [rehypeDocs],
    }),
    syntaxHighlight: 'shiki',
    // Farben über CSS-Variablen (--astro-code-*), gesetzt in src/styles/docs.css.
    shikiConfig: { theme: 'css-variables' },
  },
  vite: {
    build: { assetsInlineLimit: 0 },
    // docs/ liegt außerhalb von website/; dev und preview dürfen dorthin lesen.
    server: { fs: { allow: [repoRoot] } },
  },
});
