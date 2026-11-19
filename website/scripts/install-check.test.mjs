// Die Prüfung von /install und der Leseseite (HUM-528) an erfundenen Eingaben: Sie muss jede
// Abweichung finden, sonst schützt der Lauf nach dem Bau nichts.
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { htmlText, problems, scripts, shownSource } from './install-check.mjs';

const script = Buffer.from('#!/bin/sh\n# a < b && c > "d" \'e\'\nmain "$@"\n');
// So gibt Shiki den Block aus: Zeilen als Spans, Sonderzeichen als Entitäten.
const page = (body) =>
  `<html><body><div class="code" data-install-source><div class="code__bar">sh</div><pre class="astro-code" tabindex="0"><code>${body}</code></pre></div></body></html>`;
const good = page(
  '<span class="line"><span style="color:x">#!/bin/sh</span></span>\n' +
    '<span class="line"><span># a &lt; b &amp;&amp; c &gt; &quot;d&quot; &#39;e&#39;</span></span>\n' +
    '<span class="line"><span>main </span><span>&#x22;$@&#x22;</span></span>',
);

test('htmlText resolves tags and entities', () => {
  assert.equal(htmlText('<b>&lt;x&gt;</b> &amp; &#39;&#x27;&quot;'), `<x> & ''"`);
});

test('the source block is found', () => {
  assert.equal(shownSource(good), script.toString().replace(/\n$/, ''));
  assert.equal(shownSource('<pre>other</pre>'), null);
});

test('identical file and pages: no problem', () => {
  assert.deepEqual(problems(script, Buffer.from(script), [{ name: 'de', html: good }, { name: 'en', html: good }]), []);
});

test('one changed byte in /install is found', () => {
  const served = Buffer.from(script);
  served[3] ^= 1;
  assert.match(problems(script, served, [])[0], /differs/);
  assert.match(problems(script, Buffer.concat([script, Buffer.from('\n')]), [])[0], /differs/);
  assert.match(problems(script, null, [])[0], /missing/);
});

test('a page with different text, without a block or missing is found', () => {
  const changed = good.replace('main ', 'main  ');
  assert.match(problems(script, script, [{ name: 'de', html: changed }])[0], /different text, first at line 3/);
  assert.match(problems(script, script, [{ name: 'de', html: '<p>nothing</p>' }])[0], /no source block/);
  assert.match(problems(script, script, [{ name: 'en', html: null }])[0], /missing/);
});

test('uninstall.sh is checked like install.sh, under its own names (HUM-550)', () => {
  assert.deepEqual(
    scripts.map((entry) => [entry.file, entry.served]),
    [
      ['install.sh', 'install'],
      ['uninstall.sh', 'uninstall'],
    ],
  );
  assert.deepEqual(scripts[1].pages, ['deinstallationsskript/index.html', 'en/uninstall-script/index.html']);
  const served = Buffer.from(script);
  served[0] ^= 1;
  assert.match(problems(script, served, [], scripts[1])[0], /dist\/uninstall differs from uninstall\.sh/);
  assert.match(problems(script, null, [], scripts[1])[0], /dist\/uninstall is missing/);
});
