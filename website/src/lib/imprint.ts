// Das Impressum liegt nicht in diesem Repository. Der offizielle Bau (nurkert/Humanitl)
// setzt IMPRINT_URL und holt beim Bauen die eine Quelle des Inhabers; Forks setzen ihre
// eigene Adresse oder zeigen einen Hinweis. Das geholte Markup wird auf wenige Tags reduziert.

const allowedTags = new Set(['p', 'br', 'strong', 'b', 'em', 'i', 'h2', 'h3', 'ul', 'li']);

function escapeText(text: string): string {
  return text.replace(/&(?!(?:[a-z]+|#\d+|#x[\da-f]+);)/gi, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

/** Behält nur erlaubte Tags, entfernt jedes Attribut und maskiert alles andere. */
export function sanitize(html: string): string {
  const withoutBlocks = html.replace(/<(script|style|iframe|object|template)[\s\S]*?<\/\1>/gi, '');
  return withoutBlocks
    .split(/(<[^>]*>)/)
    .map((part) => {
      const tag = part.match(/^<\s*(\/?)\s*([a-z0-9]+)[^>]*>$/i);
      if (!tag) return part.startsWith('<') ? '' : escapeText(part);
      const name = tag[2].toLowerCase();
      if (!allowedTags.has(name)) return '';
      if (name === 'br') return '<br>';
      return `<${tag[1]}${name}>`;
    })
    .join('')
    .replace(/\n\s+/g, '\n')
    .trim();
}

export interface Imprint {
  html: string;
  source: string;
}

export async function loadImprint(): Promise<Imprint | null> {
  const source = process.env.IMPRINT_URL?.trim();
  if (!source) return null;
  const url = new URL(source);
  if (url.protocol !== 'https:') throw new Error('IMPRINT_URL must use https');

  const response = await fetch(url, { signal: AbortSignal.timeout(15_000), redirect: 'error' });
  if (!response.ok) throw new Error(`Imprint could not be loaded (${response.status}) from ${url}`);
  const page = await response.text();

  // Der Inhalt nach der Überschrift <h1> bis zum Ende des Body.
  const afterHeading = page.split(/<\/h1>/i)[1] ?? '';
  const body = afterHeading.split(/<\/body>/i)[0];
  const html = sanitize(body);
  if (html.replace(/<[^>]+>/g, '').trim().length < 40) {
    throw new Error(`Imprint at ${url} looks empty; refusing to publish without an imprint`);
  }
  return { html, source: url.toString() };
}
