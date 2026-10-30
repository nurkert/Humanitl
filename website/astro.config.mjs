import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://humanitl.de',
  trailingSlash: 'always',
  build: {
    // CSS und JS liegen in Dateien, damit die Content-Security-Policy ohne Inline-Skripte auskommt.
    inlineStylesheets: 'never',
  },
  vite: {
    build: { assetsInlineLimit: 0 },
  },
});
