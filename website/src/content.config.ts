import { glob } from 'astro/loaders';
import { defineCollection } from 'astro:content';

// Die Dokumentation ist der Ordner docs/ des Repositorys, beim Bau unverändert gelesen
// (HUM-336). Kein Schema: die Dateien haben kein Front Matter. Die Übersetzungen unter
// docs/translations/ sind keine eigenen Seiten, sondern eine eigene Sammlung (HUM-346).
const docs = defineCollection({
  loader: glob({ pattern: ['**/*.md', '!translations/**'], base: '../docs' }),
});

// Übersetzungen: docs/translations/<lang>/<Pfad wie im Original>, mit Front Matter `source`,
// `source-sha256` und `lang`. Bewusst ohne Schema: eine Übersetzung mit kaputtem Front Matter
// darf den Bau nicht anhalten (andere Sitzungen ändern die Docs); src/lib/docs.ts prüft die
// Felder und übergeht eine solche Datei mit einer Warnung, scripts/check-translations.mjs meldet sie.
const translations = defineCollection({
  loader: glob({ pattern: '**/*.md', base: '../docs/translations' }),
});

export const collections = { docs, translations };
