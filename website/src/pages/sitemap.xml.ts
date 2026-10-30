// Die Sitemap aus der Sprachkonfiguration, mit hreflang-Verweisen, dazu die Docs-Seiten
// (HUM-336), die es seit HUM-346 in beiden Sprachen gibt, ebenfalls mit hreflang-Paaren.
import type { APIRoute } from 'astro';
import { getCollection } from 'astro:content';

import { alternates, defaultLang, pages, path, type Page } from '../i18n/ui';
import { docAlternates } from '../lib/docs';

export const GET: APIRoute = async ({ site }) => {
  const url = (href: string) => new URL(href, site).toString();
  const pair = (versions: { lang: string; href: string }[], fallback: string) => {
    const links = [
      ...versions.map((v) => `<xhtml:link rel="alternate" hreflang="${v.lang}" href="${url(v.href)}"/>`),
      `<xhtml:link rel="alternate" hreflang="x-default" href="${url(fallback)}"/>`,
    ].join('');
    return versions.map((v) => `<url><loc>${url(v.href)}</loc>${links}</url>`);
  };
  const entries = (Object.keys(pages) as Page[]).flatMap((page) => pair(alternates(page), path(page, defaultLang)));
  const docs = (await getCollection('docs'))
    .map((entry) => entry.id)
    .sort()
    .flatMap((id) => {
      const versions = docAlternates(id);
      return pair(versions, versions.find((v) => v.lang === defaultLang)!.href);
    });
  entries.push(...docs);
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${entries.join('\n')}
</urlset>`;
  return new Response(body, { headers: { 'Content-Type': 'application/xml; charset=utf-8' } });
};
