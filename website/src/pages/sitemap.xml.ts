// Die Sitemap aus der Sprachkonfiguration, mit hreflang-Verweisen.
import type { APIRoute } from 'astro';

import { alternates, defaultLang, pages, path, type Page } from '../i18n/ui';

export const GET: APIRoute = ({ site }) => {
  const url = (href: string) => new URL(href, site).toString();
  const entries = (Object.keys(pages) as Page[]).flatMap((page) => {
    const versions = alternates(page);
    const links = [
      ...versions.map((v) => `<xhtml:link rel="alternate" hreflang="${v.lang}" href="${url(v.href)}"/>`),
      `<xhtml:link rel="alternate" hreflang="x-default" href="${url(path(page, defaultLang))}"/>`,
    ].join('');
    return versions.map((v) => `<url><loc>${url(v.href)}</loc>${links}</url>`);
  });
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${entries.join('\n')}
</urlset>`;
  return new Response(body, { headers: { 'Content-Type': 'application/xml; charset=utf-8' } });
};
