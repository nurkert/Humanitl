// Tests der Markdown-Plugins für die Docs (HUM-336), an einem Fixture statt an den echten
// Dateien: Hinweisboxen kommen in docs/ heute nicht vor und müssen trotzdem funktionieren.
//
//   node --test scripts/
import assert from 'node:assert/strict';
import path from 'node:path';
import { test } from 'node:test';
import { pathToFileURL } from 'node:url';

import { createMarkdownProcessor } from '@astrojs/markdown-remark';

import { docsRoot } from '../src/lib/docs-catalog.mjs';
import rehypeDocs from '../src/lib/rehype-docs.mjs';
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
