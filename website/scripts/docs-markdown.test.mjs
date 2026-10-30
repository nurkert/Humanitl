// Tests der Markdown-Plugins für die Docs (HUM-336), an einem Fixture statt an den echten
// Dateien: Hinweisboxen kommen in docs/ heute nicht vor und müssen trotzdem funktionieren.
//
//   node --test scripts/
import assert from 'node:assert/strict';
import path from 'node:path';
import { test } from 'node:test';
import { pathToFileURL } from 'node:url';

import { createMarkdownProcessor } from '@astrojs/markdown-remark';

import { docsRoot, localizeDocLinks, translationInfo } from '../src/lib/docs-catalog.mjs';
import rehypeDocs, { alignHeadings } from '../src/lib/rehype-docs.mjs';
import remarkDocs from '../src/lib/remark-docs.mjs';

const processor = await createMarkdownProcessor({
  smartypants: false,
  remarkPlugins: [remarkDocs],
  rehypePlugins: [rehypeDocs],
  syntaxHighlight: 'shiki',
  shikiConfig: { theme: 'css-variables' },
});

/** Rendert Markdown so, als läge es unter docs/<file>; die Datei muss nicht existieren. */
async function render(markdown, file = 'guides/fixture.md') {
  const { code, metadata } = await processor.render(markdown, {
    fileURL: pathToFileURL(path.join(docsRoot, file)),
  });
  return { html: code, headings: metadata.headings };
}

test('GitHub alerts become boxes with title, for every type', async () => {
  for (const [type, title] of [
    ['NOTE', 'Note'],
    ['TIP', 'Tip'],
    ['IMPORTANT', 'Important'],
    ['WARNING', 'Warning'],
    ['CAUTION', 'Caution'],
  ]) {
    const { html } = await render(`> [!${type}]\n> Keep the **daemon** running.\n`);
    assert.match(html, new RegExp(`<div class="alert alert--${type.toLowerCase()}" role="note">`));
    assert.match(html, new RegExp(`<p class="alert__title"><svg[^>]*>.*?</svg>${title}</p>`));
    assert.match(html, /Keep the <strong>daemon<\/strong> running\./);
    assert.doesNotMatch(html, /\[!/);
  }
});

test('a plain blockquote stays a blockquote', async () => {
  const { html } = await render('> Purpose: say what matters.\n');
  assert.match(html, /<blockquote>/);
  assert.doesNotMatch(html, /alert/);
});

test('relative links: docs to site routes, other repo files to GitHub', async () => {
  const { html } = await render(
    [
      '[a](../UX.md#33-colour-means-state)',
      '[b](install.md)',
      '[c](../README.md)',
      '[d](../adr/README.md)',
      '[e](../../tests/escape/)',
      '[f](../../app/packages/ui)',
      '[g](../../rules/default.yaml)',
      '[h](../../backlog/CONVENTIONS.md#39-flutter-app)',
      '[i](https://example.org/x.md)',
      '[j](#local)',
      '<https://github.com/nurkert/Humanitl/security/advisories/new>',
      '`../UX.md` stays code',
    ].join('\n\n'),
  );
  assert.match(html, /href="\/docs\/ux\/#33-colour-means-state"/);
  assert.match(html, /href="\/docs\/guides\/install\/"/);
  assert.match(html, /href="\/docs\/"/);
  assert.match(html, /href="\/docs\/adr\/"/);
  assert.match(html, /href="https:\/\/github.com\/nurkert\/Humanitl\/tree\/main\/tests\/escape\/"/);
  assert.match(html, /href="https:\/\/github.com\/nurkert\/Humanitl\/tree\/main\/app\/packages\/ui"/);
  assert.match(html, /href="https:\/\/github.com\/nurkert\/Humanitl\/blob\/main\/rules\/default.yaml"/);
  assert.match(html, /href="https:\/\/github.com\/nurkert\/Humanitl\/blob\/main\/backlog\/CONVENTIONS.md#39-flutter-app"/);
  assert.match(html, /href="https:\/\/example.org\/x.md"/);
  assert.match(html, /href="#local"/);
  assert.match(html, /href="https:\/\/github.com\/nurkert\/Humanitl\/security\/advisories\/new"/);
  assert.match(html, /<code>..\/UX.md<\/code> stays code/);
});

test('headings get GitHub slugs and an anchor link', async () => {
  const { html, headings } = await render(
    '# T\n\n## ADR-0001 · Daemon — in Rust\n\n### 2.10 Reduced motion\n\n#### DAEMON_001\n\n## `--json`\n\n## `--json`\n',
  );
  const slugs = headings.map((heading) => heading.slug);
  assert.deepEqual(slugs, ['t', 'adr-0001--daemon--in-rust', '210-reduced-motion', 'daemon_001', '--json', '--json-1']);
  assert.match(html, /<div class="hwrap hwrap--h2"><h2 id="adr-0001--daemon--in-rust">/);
  assert.match(html, /<a class="anchor" href="#210-reduced-motion">/);
});

test('meta lines under the H1 become chips without changing the text', async () => {
  const { html } = await render('# ADR-0001 · X\n\nApplies to: Humanitl 0.1\n\nStatus: Accepted\nDate: 2026-09-02\n\nBody.\n', 'adr/0001-x.md');
  assert.match(html, /<span class="chip"><span class="chip__k">Applies to:<\/span> Humanitl 0.1<\/span>/);
  assert.match(html, /<span class="chip" data-status="accepted"><span class="chip__k">Status:<\/span> Accepted<\/span>/);
  assert.match(html, /<span class="chip__k">Date:<\/span> 2026-09-02/);
  assert.match(html, /<p>Body.<\/p>/);
});

test('comments vanish, lang sections get a marker class without lang, punctuation stays as written', async () => {
  const { html } = await render(
    'Before -- "quoted" it\'s.\n\n<!-- a remark -->\n\n<!-- lang: de -->\n| a | b |\n|---|---|\n| Warteschlange | x |\n<!-- /lang -->\n',
  );
  assert.doesNotMatch(html, /<!--/);
  assert.match(html, /<div (?:class="lang-block" data-lang="de"|data-lang="de" class="lang-block")>/);
  assert.doesNotMatch(html, /\slang="de"/);
  assert.match(html, /Before -- "quoted" it's\./);
  assert.match(html, /<div class="table-wrap" role="region" tabindex="0" aria-label="Table"><table>/);
});

test('code fences become a figure, with a language bar only for named languages', async () => {
  const { html } = await render('```sh\n$ humanitl doctor\n```\n\n```\nplain\n```\n');
  assert.match(html, /<figure class="code code--bar" data-lang="sh"><figcaption class="code__bar"><span class="code__lang">Shell<\/span>/);
  assert.match(html, /<figure class="code code--bare" data-lang="plaintext"><pre/);
});

test('raw HTML outside the GitHub allowlist is unwrapped, its text stays', async () => {
  const { html } = await render(
    'Intro `a\n<dir>` b.\n\n<dir>\nopen tag\n\n## Later\n\nText <kbd>K</kbd> <script>alert(1)</script>.\n\n- item\n',
  );
  assert.doesNotMatch(html, /<\/?dir\b/);
  assert.doesNotMatch(html, /<script/);
  assert.match(html, /<kbd>K<\/kbd>/);
  assert.match(html, /<ul>\s*<li>item<\/li>\s*<\/ul>/);
});

test('raw HTML keeps only allowed attributes and safe targets, code keeps its colours', async () => {
  const { html } = await render(
    [
      '<div style="position:fixed;inset:0" data-x="1" title="ok">Cover</div>',
      '',
      '<a href="java&#x09;script:alert(1)">bad</a> <a href="https://example.org" onclick="x()">good</a>',
      '',
      '```sh',
      'echo hi # comment',
      '```',
    ].join('\n'),
  );
  assert.doesNotMatch(html, /position:fixed/);
  assert.doesNotMatch(html, /data-x/);
  assert.match(html, /title="ok"/);
  assert.doesNotMatch(html, /script:alert/);
  assert.doesNotMatch(html, /onclick/);
  assert.match(html, /href="https:\/\/example\.org"/);
  // Shiki färbt über style-Attribute innerhalb von <pre>; die bleiben.
  assert.match(html, /<pre[^>]*>[\s\S]*style="[^"]*--astro-code/);
});

test('links: files without extension go to blob, query and anchor stay with the doc route', async () => {
  const { html } = await render('[licence](../../LICENSE) [ux](../UX.md?x=1#hello)');
  assert.match(html, /href="https:\/\/github\.com\/nurkert\/Humanitl\/blob\/main\/LICENSE"/);
  assert.match(html, /href="\/docs\/ux\/\?x=1#hello"/);
});

// ------------------------------------------------------------------ Übersetzungen (HUM-346)

// Originale für die Übersetzungs-Tests, statt der echten Dateien unter docs/.
const fixtureSources = new Map([
  [
    path.join(docsRoot, 'guides/fixture.md'),
    '# Fixture guide\n\nApplies to: Humanitl 0.1\n\n## Without the app\n\n### Step `one`\n\n## Without the app\n\n## What the chain proves\n',
  ],
  [
    path.join(docsRoot, 'guides/numbered.md'),
    '# Numbered\n\n## 3.1 Colour\n\n## 3.2 Motion\n\n## 3.3 Type\n\n## Appendix\n',
  ],
  [path.join(docsRoot, 'adr/0099-fixture.md'), '# ADR-0099 · Fixture\n\nApplies to: Humanitl 0.1\n\nStatus: Accepted\nDate: 2026-10-30\n\n## Context\n'],
]);

const translationProcessor = await createMarkdownProcessor({
  smartypants: false,
  remarkPlugins: [remarkDocs],
  rehypePlugins: [
    [
      rehypeDocs,
      {
        readSource: async (file) => {
          if (!fixtureSources.has(file)) throw Object.assign(new Error('missing'), { code: 'ENOENT' });
          return fixtureSources.get(file);
        },
      },
    ],
  ],
  syntaxHighlight: 'shiki',
  shikiConfig: { theme: 'css-variables' },
});

/** Rendert eine Übersetzung so, als läge sie unter docs/translations/<lang>/<source>. */
async function renderTranslation(markdown, source = 'guides/fixture.md', lang = 'de') {
  const file = path.join(docsRoot, 'translations', lang, source);
  const warnings = [];
  const warn = console.warn;
  console.warn = (message) => warnings.push(String(message));
  try {
    const { code, metadata } = await translationProcessor.render(markdown, {
      fileURL: pathToFileURL(file),
      frontmatter: { source, 'source-sha256': '0'.repeat(64), lang },
    });
    return { html: code, headings: metadata.headings, frontmatter: metadata.frontmatter, warnings };
  } finally {
    console.warn = warn;
  }
}

test('a translation takes the heading ids of its original, by position', async () => {
  const { html, headings, warnings } = await renderTranslation(
    '# Leitfaden\n\nApplies to: Humanitl 0.1\n\n## Ohne die App\n\n### Schritt `eins`\n\n## Ohne die App\n\n## Was die Kette beweist\n',
  );
  assert.deepEqual(
    headings.map((heading) => [heading.depth, heading.slug, heading.text]),
    [
      [1, 'fixture-guide', 'Leitfaden'],
      [2, 'without-the-app', 'Ohne die App'],
      [3, 'step-one', 'Schritt eins'],
      [2, 'without-the-app-1', 'Ohne die App'],
      [2, 'what-the-chain-proves', 'Was die Kette beweist'],
    ],
  );
  assert.match(html, /<h2 id="without-the-app">Ohne die App<\/h2><a class="anchor" href="#without-the-app">/);
  assert.match(html, /Abschnitt „Was die Kette beweist“/);
  assert.match(html, /<span class="chip__k">Gilt für:<\/span> Humanitl 0.1/);
  assert.deepEqual(warnings, []);
});

test('a translation with a different structure keeps its own ids and is reported', async () => {
  const fewer = await renderTranslation('# Leitfaden\n\n## Ohne die App\n\n## Was die Kette beweist\n');
  assert.deepEqual(
    fewer.headings.map((heading) => heading.slug),
    ['leitfaden', 'ohne-die-app', 'was-die-kette-beweist'],
  );
  assert.equal(fewer.warnings.length, 1);
  assert.match(fewer.warnings[0], /translations\/de\/guides\/fixture\.md: 3 headings, the original has 5/);

  const level = await renderTranslation('# L\n\n## A\n\n## B\n\n## C\n\n## D\n');
  assert.deepEqual(level.headings.map((heading) => heading.slug), ['l', 'a', 'b', 'c', 'd']);
  assert.match(level.warnings[0], /heading 3 is h2, in the original h3/);

  const orphan = await renderTranslation('# Waise\n', 'guides/gone.md');
  assert.deepEqual(orphan.headings.map((heading) => heading.slug), ['waise']);
  assert.match(orphan.warnings[0], /original not readable \(ENOENT\)/);
});

/** Alle IDs im HTML, in Reihenfolge. */
const idsIn = (html) => [...html.matchAll(/\sid="([^"]*)"/g)].map((match) => match[1]);

test('fallback: every id of the original stays reachable as an empty anchor, nothing is duplicated', async () => {
  // Das Original hat eine H3 und eine H2 mehr als die Übersetzung (etwa, weil es seither wuchs).
  const { html, headings, frontmatter, warnings } = await renderTranslation(
    '# Leitfaden\n\n## Ohne die App\n\n## Was die Kette beweist\n',
  );
  assert.deepEqual(
    headings.map((heading) => heading.slug),
    ['leitfaden', 'ohne-die-app', 'was-die-kette-beweist'],
  );
  const ids = idsIn(html);
  for (const slug of ['fixture-guide', 'without-the-app', 'step-one', 'without-the-app-1', 'what-the-chain-proves']) {
    assert.ok(ids.includes(slug), `id ${slug} missing in ${html}`);
  }
  assert.equal(new Set(ids).size, ids.length, `duplicate ids: ${ids}`);
  assert.match(html, /<a id="fixture-guide" class="source-anchor" aria-hidden="true"><\/a><h1 id="leitfaden">/);
  assert.equal(frontmatter.headingFallback, true);
  assert.match(warnings[0], /3 headings, the original has 5; the page keeps its own heading ids, 5 anchor\(s\) of the original added/);
});

test('fallback: a heading the original gained gets its anchor where it would stand, matched by section numbers', async () => {
  // Die Übersetzung kennt 3.2 noch nicht; die übrigen Abschnitte tragen dieselben Nummern.
  const { html, warnings } = await renderTranslation(
    '# Nummeriert\n\n## 3.1 Farbe\n\n## 3.3 Schrift\n\n## Anhang\n',
    'guides/numbered.md',
  );
  assert.equal(warnings.length, 1);
  // 3.2 steht vor 3.3, nicht am Ende und nicht vor 3.1.
  assert.match(html, /<a id="32-motion"[^>]*><\/a><a id="33-type"[^>]*><\/a><div class="hwrap hwrap--h2"><h2 id="33-schrift">/);
  assert.match(html, /<a id="31-colour"[^>]*><\/a><div class="hwrap hwrap--h2"><h2 id="31-farbe">/);
  assert.match(html, /<a id="appendix"[^>]*><\/a><div class="hwrap hwrap--h2"><h2 id="anhang">/);
});

test('alignHeadings: by level, numbers decide, an added heading at the start shifts nothing', () => {
  const h = (depth, digits = '') => ({ depth, digits });
  // Original gewann vorne eine H2 ohne Nummer: Zuordnung über die Nummern bleibt richtig.
  assert.deepEqual(alignHeadings([h(1), h(2, '1'), h(2, '2')], [h(1), h(2), h(2, '1'), h(2, '2')]), [0, -1, 1, 2]);
  // Ebenen müssen passen.
  assert.deepEqual(alignHeadings([h(1), h(3)], [h(1), h(2), h(3)]), [0, -1, 1]);
  assert.deepEqual(alignHeadings([], [h(1)]), [-1]);
});

test('links in a translation resolve against the original, images against the same file', async () => {
  const { html } = await renderTranslation(
    [
      '# Leitfaden',
      '',
      '[a](../UX.md#33-colour-means-state) [b](install.md) [c](../README.md#using-humanitl)',
      '',
      '[d](../../tests/escape/) [e](#without-the-app) [f](https://example.org/x.md)',
      '',
      '![Palette](img/palette.png)',
    ].join('\n'),
    'guides/fixture.md',
  );
  assert.match(html, /href="\/docs\/ux\/#33-colour-means-state"/);
  assert.match(html, /href="\/docs\/guides\/install\/"/);
  assert.match(html, /href="\/docs\/#using-humanitl"/);
  assert.match(html, /href="https:\/\/github.com\/nurkert\/Humanitl\/tree\/main\/tests\/escape\/"/);
  assert.match(html, /href="#without-the-app"/);
  assert.match(html, /href="https:\/\/example.org\/x.md"/);
  // Das Bild des Originals docs/guides/img/palette.png, vom Ordner der Übersetzung aus.
  // Astro legt lokale Bilder als Platzhalter `__ASTRO_IMAGE_` mit dem Pfad als JSON ab.
  assert.match(html, /&#x22;src&#x22;:&#x22;\.\.\/\.\.\/\.\.\/guides\/img\/palette\.png&#x22;/);
});

test('an English translation of a German original: links stay German until localised, words are English', async () => {
  const { html } = await renderTranslation(
    '# ADR-0099 · Fixture\n\nApplies to: Humanitl 0.1\n\nStatus: Accepted\nDate: 2026-10-30\n\n## Context\n\nSee [config](../CONFIG.md#stufen).\n',
    'adr/0099-fixture.md',
    'en',
  );
  assert.match(html, /<span class="chip__k">Applies to:<\/span>/);
  assert.match(html, /<span class="chip" data-status="accepted"><span class="chip__k">Status:<\/span> Accepted<\/span>/);
  assert.match(html, /<h2 id="context">/);
  assert.match(html, /href="\/docs\/config\/#stufen"/);
  const english = localizeDocLinks(html, 'en');
  assert.match(english, /href="\/en\/docs\/config\/#stufen"/);
  assert.doesNotMatch(english, /href="\/docs\//);
});

test('German words in German documents: meta keys, status values and alerts', async () => {
  const { html } = await renderTranslation(
    '# ADR-0099 · Beispiel\n\nApplies to: Humanitl 0.1\n\nStatus: Accepted\nDate: 2026-10-30\n\n## Kontext\n\n> [!WARNING]\n> Achtung.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n',
    'adr/0099-fixture.md',
  );
  assert.match(html, /<span class="chip__k">Status:<\/span> Angenommen<\/span>/);
  assert.match(html, /<span class="chip__k">Datum:<\/span> 2026-10-30/);
  assert.match(html, /<p class="alert__title"><svg[^>]*>.*?<\/svg>Warnung<\/p>/);
  assert.match(html, /aria-label="Tabelle: Kontext"/);
});

test('localizeDocLinks moves only docs links, and leaves code untouched', () => {
  const html = '<a href="/docs/">a</a><a href="/docs/ux/#x">b</a><a href="/download/">c</a><code>href=&quot;/docs/&quot;</code>';
  assert.equal(localizeDocLinks(html, 'de'), html);
  assert.equal(
    localizeDocLinks(html, 'en'),
    '<a href="/en/docs/">a</a><a href="/en/docs/ux/#x">b</a><a href="/download/">c</a><code>href=&quot;/docs/&quot;</code>',
  );
});

test('translationInfo: language and original come from the path, never from the front matter', () => {
  const file = path.join(docsRoot, 'translations/de/guides/install.md');
  assert.deepEqual(translationInfo(file, { source: 'guides/install.md', lang: 'de' }), {
    lang: 'de',
    source: 'guides/install.md',
    sourcePath: path.join(docsRoot, 'guides/install.md'),
  });
  assert.equal(translationInfo(file, { source: '../../etc/passwd.md' }).source, 'guides/install.md');
  assert.equal(translationInfo(file, { source: 'translations/en/CONFIG.md' }).source, 'guides/install.md');
  assert.equal(translationInfo(file, { lang: 'fr' }).lang, 'de');
  assert.equal(translationInfo(file, { source: 'ARCHITECTURE.md', lang: 'en' }).source, 'guides/install.md');
  assert.equal(translationInfo(file, { source: 'ARCHITECTURE.md', lang: 'en' }).lang, 'de');
  assert.equal(translationInfo(path.join(docsRoot, 'guides/install.md')), null);
});
