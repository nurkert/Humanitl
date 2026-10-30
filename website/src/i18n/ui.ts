// Sprachen und lokalisierte Wege. Eine neue Sprache: src/i18n/locales/<code>.ts anlegen
// (gegen de.ts typisiert, damit nichts fehlen kann) und unten in `languages` eintragen.
import type { Copy } from './locales/de';
import de from './locales/de';
import en from './locales/en';

export const defaultLang = 'de';

export const languages = {
  de: { label: 'Deutsch', short: 'DE', locale: 'de-DE', og: 'de_DE', copy: de },
  en: { label: 'English', short: 'EN', locale: 'en-GB', og: 'en_GB', copy: en },
} satisfies Record<string, { label: string; short: string; locale: string; og: string; copy: Copy }>;

export type Lang = keyof typeof languages;
export const langs = Object.keys(languages) as Lang[];

/** Seiten, die es in jeder Sprache gibt, mit ihrem Pfad je Sprache ('' = Startseite). */
export const pages = {
  home: { de: '', en: '' },
  download: { de: 'download', en: 'download' },
  privacy: { de: 'datenschutz', en: 'privacy' },
  imprint: { de: 'impressum', en: 'imprint' },
} satisfies Record<string, Record<Lang, string>>;

export type Page = keyof typeof pages;

export const repoUrl = 'https://github.com/nurkert/Humanitl';
export const releasesUrl = `${repoUrl}/releases`;
export const securityUrl = `${repoUrl}/blob/main/docs/SECURITY.md`;

export const ui = Object.fromEntries(langs.map((lang) => [lang, languages[lang].copy])) as Record<Lang, Copy>;

/** Der URL-Pfad einer Seite: die Standardsprache an der Wurzel, die anderen unter /<lang>/. */
export function path(page: Page, lang: Lang): string {
  const prefix = lang === defaultLang ? '' : `/${lang}`;
  const slug = pages[page][lang];
  return `${prefix}/${slug ? `${slug}/` : ''}`;
}

/** Der Parameter für Astros [...route] (undefined für die Startseite der Standardsprache). */
export function routeParam(page: Page, lang: Lang): string | undefined {
  return path(page, lang).replace(/^\/|\/$/g, '') || undefined;
}

/** Alle Sprachfassungen einer Seite, für hreflang und die Sprachwahl. */
export function alternates(page: Page): { lang: Lang; href: string }[] {
  return langs.map((lang) => ({ lang, href: path(page, lang) }));
}
