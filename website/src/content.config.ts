import { glob } from 'astro/loaders';
import { defineCollection } from 'astro:content';

// Die Dokumentation ist der Ordner docs/ des Repositorys, beim Bau unverändert gelesen
// (HUM-336). Kein Schema: die Dateien haben kein Front Matter.
const docs = defineCollection({
  loader: glob({ pattern: '**/*.md', base: '../docs' }),
});

export const collections = { docs };
