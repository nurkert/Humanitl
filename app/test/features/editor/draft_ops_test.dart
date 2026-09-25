/// Die reinen Funktionen des Editors (HUM-047).
///
/// Kein Widget, kein Provider: Was der Editor mit einem Entwurf tut, ist hier
/// vollständig prüfbar, und jede Zusage der Spezifikation steht als ein Test
/// da.
library;

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/domain/http_request.dart';
import 'package:humanitl/features/editor/model/draft.dart';
import 'package:humanitl/features/editor/model/draft_ops.dart';
import 'package:humanitl/features/editor/model/pseudonym_naming.dart';
import 'package:humanitl/features/editor/providers/draft_provider.dart';

/// Ein Fund über `[start, end)` im Rumpf.
FindingView bodyFinding(
  int index,
  String kind,
  int start,
  int end, {
  String hash = '',
  FindingTier tier = FindingTier.regex,
}) => FindingView(
  index: index,
  finding: Finding(
    kind: kind,
    location: FindingLocation.body,
    spanStart: start,
    spanEnd: end,
    tier: tier,
  ),
  location: DraftLocation.body,
  start: start,
  end: end,
  originalStart: start,
  originalEnd: end,
  valueHash: hash.isEmpty ? 'hash-$index' : hash,
);

/// Ein Fund im Wert einer Kopfzeile.
FindingView headerFinding(
  int index,
  String kind,
  String header,
  int start,
  int end, {
  String hash = '',
}) => FindingView(
  index: index,
  finding: Finding(
    kind: kind,
    location: FindingLocation.header,
    headerName: header,
    spanStart: start,
    spanEnd: end,
    tier: FindingTier.regex,
  ),
  location: DraftLocation.header(header),
  start: start,
  end: end,
  originalStart: start,
  originalEnd: end,
  valueHash: hash.isEmpty ? 'hash-$index' : hash,
);

Draft draftOf({
  String body = '',
  BodyKind kind = BodyKind.text,
  List<FindingView> findings = const <FindingView>[],
  List<HeaderEntry> headers = const <HeaderEntry>[],
  String pathAndQuery = '/v1/chat',
}) => Draft(
  flowId: const FlowId('01930000-0000-7000-8000-000000000001'),
  method: 'POST',
  scheme: Scheme.https,
  authority: const Authority(host: 'api.example.com', port: 443),
  pathAndQuery: pathAndQuery,
  bodyKind: kind,
  body: body,
  headers: headers,
  findings: findings,
);

void main() {
  group('replaceFinding', () {
    test('replaceFinding_shiftsLaterSpans', () {
      // "a@x.de und b@y.de": der zweite Fund beginnt bei 11.
      const String body = 'a@x.de und b@y.de';
      expect(body.indexOf('b@y.de'), 11);
      final Draft before = draftOf(
        body: body,
        findings: <FindingView>[
          bodyFinding(0, 'email', 0, 6),
          bodyFinding(1, 'email', 11, 17),
        ],
      );

      final Draft after = replaceFinding(before, 0, '<EMAIL_1>');

      expect(after.body, '<EMAIL_1> und b@y.de');
      final FindingView second = after.findings[1];
      // Alter Start plus die Laengendifferenz, nicht neu gesucht.
      expect(second.start, 11 + ('<EMAIL_1>'.length - 'a@x.de'.length));
      expect(second.start, 14);
      expect(after.body.substring(second.start, second.end), 'b@y.de');
      expect(after.findings[0].status, FindingStatus.replaced);
      expect(after.dirty, isTrue);
    });

    test('replaceFinding_overlapRemoved', () {
      // Zwei Muster, die sich ueberlappen: ein Regex-Artefakt, nie zwei Werte.
      final Draft before = draftOf(
        body: 'DE02120300000000202051 xx',
        findings: <FindingView>[
          bodyFinding(0, 'iban', 0, 22),
          bodyFinding(1, 'phone', 10, 20),
        ],
      );

      final Draft after = replaceFinding(before, 0, '<IBAN_1>');

      expect(after.findings[1].status, FindingStatus.removed);
      expect(openFindings(after), 0);
    });

    test('a replacement before an earlier one shifts the earlier glow', () {
      final Draft before = draftOf(
        body: 'a@x.de und b@y.de',
        findings: <FindingView>[
          bodyFinding(0, 'email', 0, 6),
          bodyFinding(1, 'email', 11, 17),
        ],
      );

      // Erst den hinteren ersetzen, dann den vorderen: Der Diff-Glow des
      // hinteren muss mitwandern.
      final Draft after = replaceFinding(
        replaceFinding(before, 1, '<EMAIL_2>'),
        0,
        '<EMAIL_1>',
      );

      expect(after.body, '<EMAIL_1> und <EMAIL_2>');
      final Replacement later = after.replacements.firstWhere(
        (Replacement done) => done.pseudonym == '<EMAIL_2>',
      );
      expect(after.body.substring(later.start, later.end), '<EMAIL_2>');
    });

    test('a finding that is no longer open is left alone', () {
      final Draft before = draftOf(
        body: 'a@x.de',
        findings: <FindingView>[bodyFinding(0, 'email', 0, 6)],
      );
      final Draft once = replaceFinding(before, 0, '<EMAIL_1>');

      expect(replaceFinding(once, 0, '<EMAIL_9>'), once);
    });
  });

  group('the anchor of a finding (HUM-161)', () {
    test(
      'a finding whose value no longer stands at its place is not replaced',
      () {
        // Die Offsets zeigen auf `il a@x`, der Wert steht zwei Zeichen weiter.
        final Draft draft = draftOf(
          body: 'my mail a@x.de',
          findings: <FindingView>[
            bodyFinding(0, 'email', 5, 11).copyWith(value: 'a@x.de'),
          ],
        );

        final Draft after = replaceFinding(draft, 0, '<EMAIL_1>');

        expect(after.body, 'my mail a@x.de');
        expect(openFindings(after), 1);
        final Draft all = replaceAllOpen(draft, PseudonymNaming());
        expect(all.body, 'my mail a@x.de');
        expect(openFindings(all), 1);
      },
    );

    test('reanchoring moves a finding to its value', () {
      final Draft draft = draftOf(
        body: 'my mail a@x.de',
        findings: <FindingView>[
          bodyFinding(0, 'email', 5, 11).copyWith(value: 'a@x.de'),
        ],
      );

      final Draft after = replaceAllOpen(
        reanchorFindings(draft),
        PseudonymNaming(),
      );

      expect(after.body, 'my mail <EMAIL_1>');
      expect(openFindings(after), 0);
    });

    test('an ambiguous value stays open at the first free copy', () {
      final Draft draft = draftOf(
        body: 'a@x.de xx a@x.de',
        findings: <FindingView>[
          bodyFinding(0, 'email', 8, 14).copyWith(value: 'a@x.de'),
        ],
      );

      final FindingView view = reanchorFindings(draft).findings.single;

      expect(view.status, FindingStatus.open);
      expect(view.start, 0);
    });

    test('a value gone from its place is removed, never ignored', () {
      final Draft draft = draftOf(
        body: 'mail nobody',
        findings: <FindingView>[
          bodyFinding(0, 'email', 5, 11).copyWith(value: 'a@x.de'),
        ],
      );

      final FindingView view = reanchorFindings(draft).findings.single;

      expect(view.status, FindingStatus.removed);
    });
  });

  group('anchoring after review round 4 (HUM-161)', () {
    FindingView mail(int index, int start, {String value = 'a@x.de'}) =>
        bodyFinding(
          index,
          'email',
          start,
          start + value.length,
          hash: 'mail',
        ).copyWith(value: value);

    test('two findings of one value never share a copy', () {
      // Kopien bei 3 und 13; fünf Zeichen davor verschieben beide.
      final Draft draft = draftOf(
        body: '12345aa a@x.de bb a@x.de',
        findings: <FindingView>[mail(0, 3), mail(1, 13)],
      );

      final Draft after = reanchorFindings(draft);

      expect(
        <int>[for (final FindingView v in after.findings) v.start],
        <int>[8, 18],
      );
      final Draft replaced = replaceAllOpen(after, PseudonymNaming());
      expect(replaced.body, '12345aa <EMAIL_1> bb <EMAIL_1>');
      expect(openFindings(replaced), 0);
    });

    test('a value inside a standing pseudonym is no copy', () {
      // Der Wert `EMAIL` steht nach dem Ersetzen im Pseudonym `<EMAIL_1>`;
      // das öffnet den Fund nicht wieder.
      final Draft draft = draftOf(
        body: 'id EMAIL',
        findings: <FindingView>[mail(0, 3, value: 'EMAIL')],
      );

      final Draft after = replaceAllOpen(draft, PseudonymNaming());

      expect(after.body, 'id <EMAIL_1>');
      expect(openFindings(after), 0);
    });

    test('a value deleted and typed again is open again', () {
      final Draft draft = draftOf(
        body: 'mail a@x.de',
        findings: <FindingView>[mail(0, 5)],
      );

      final Draft gone = reanchorFindings(draft.copyWith(body: 'mail a@x.d'));
      expect(gone.findings.single.status, FindingStatus.removed);
      final Draft back = reanchorFindings(gone.copyWith(body: 'mail a@x.de'));

      expect(back.findings.single.status, FindingStatus.open);
      expect(back.findings.single.start, 5);
      expect(openFindings(back), 1);
    });

    test('undo after replace all opens the finding and drops its mapping', () {
      final Draft draft = draftOf(
        body: 'mail a@x.de',
        findings: <FindingView>[mail(0, 5)],
      );
      final Draft replaced = replaceAllOpen(draft, PseudonymNaming());
      expect(replaced.body, 'mail <EMAIL_1>');

      final Draft undone = reanchorFindings(
        replaced.copyWith(body: 'mail a@x.de'),
      );

      expect(undone.findings.single.status, FindingStatus.open);
      expect(undone.replacements, isEmpty);
    });

    test('a selection that cuts into a finding never ignores it', () {
      final Draft draft = draftOf(
        body: 'mail max.muster@x.de ok',
        findings: <FindingView>[mail(0, 5, value: 'max.muster@x.de')],
      );

      final Draft after = replaceSelection(
        draft,
        DraftLocation.body,
        5,
        9,
        'name',
        PseudonymNaming(),
      );

      expect(after.findings.first.status, FindingStatus.removed);
    });

    test('a value that moved into the path stays open', () {
      final Draft draft = draftOf(
        pathAndQuery: '/v1/chat?to=a@x.de',
        findings: <FindingView>[
          FindingView(
            index: 0,
            finding: Finding(
              kind: 'email',
              location: FindingLocation.query,
              spanStart: 3,
              spanEnd: 9,
              tier: FindingTier.regex,
            ),
            location: DraftLocation.query,
            start: 3,
            end: 9,
            valueHash: 'mail',
            value: 'a@x.de',
          ),
        ],
      );

      final Draft after = reanchorFindings(
        draft.copyWith(pathAndQuery: '/v1/chatto=a@x.de'),
      );

      expect(after.findings.single.status, FindingStatus.open);
      expect(openFindings(after), 1);
      final Draft replaced = replaceAllOpen(after, PseudonymNaming());
      expect(replaced.pathAndQuery, '/v1/chatto=a@x.de');
      expect(openFindings(replaced), 1);
    });
  });

  group('anchoring after review round 5 (HUM-161)', () {
    FindingView mail(int index, int start, {String value = 'a@x.de'}) =>
        bodyFinding(
          index,
          'email',
          start,
          start + value.length,
          hash: 'mail',
        ).copyWith(value: value);

    Draft unplacedDraft() => buildDraft(
      const FlowId('01930000-0000-7000-8000-00000000000f'),
      DraftSource(
        request: HttpRequest(
          method: Method.post,
          scheme: Scheme.https,
          authority: const Authority(host: 'api.example.com', port: 443),
          pathAndQuery: '/v1/chat',
          headers: const <Header>[],
          body: const BodyRef(sha256: <int>[], size: 0),
        ),
        findings: <Finding>[
          Finding(
            kind: 'email',
            location: FindingLocation.body,
            spanStart: 5,
            spanEnd: 11,
            tier: FindingTier.checksum,
          ),
        ],
        bodyText: 'mail',
        bodyKind: BodyKind.text,
        bodyBytes: Uint8List.fromList(utf8.encode('mail a@x.de')),
      ),
    );

    test('a selection over the old span of a finding without a place keeps '
        'it open', () {
      final Draft draft = unplacedDraft().copyWith(body: 'hello world');

      final Draft after = replaceSelection(
        draft,
        DraftLocation.body,
        5,
        11,
        'name',
        PseudonymNaming(),
      );

      expect(openFindings(after), 1);
    });

    test('ctrl+r then undo opens the selection again', () {
      final Draft draft = draftOf(body: 'acme corp writes');
      final Draft replaced = replaceSelection(
        draft,
        DraftLocation.body,
        0,
        9,
        'client',
        PseudonymNaming(),
      );
      expect(openFindings(replaced), 0);

      final Draft undone = reanchorFindings(
        replaced.copyWith(body: 'acme corp writes'),
      );

      expect(openFindings(undone), 1);
    });

    test('a value in the method keeps its finding open', () {
      final Draft draft = draftOf(
        body: 'mail a@x.de',
        findings: <FindingView>[mail(0, 5)],
      );

      final Draft after = reanchorFindings(
        draft.copyWith(body: 'mail', method: 'POSTa@x.de'),
      );

      expect(openFindings(after), 1);
    });

    test('a value in a header name keeps its finding open', () {
      final Draft draft = draftOf(
        body: 'mail a@x.de',
        findings: <FindingView>[mail(0, 5)],
        headers: const <HeaderEntry>[HeaderEntry(name: 'a@x.de', value: '1')],
      );

      final Draft after = reanchorFindings(draft.copyWith(body: 'mail'));

      expect(openFindings(after), 1);
    });

    test('a query value written out in plain keeps its finding open', () {
      final Draft draft = draftOf(
        pathAndQuery: '/v1/chat?to=user%40x.de',
        findings: <FindingView>[
          FindingView(
            index: 0,
            finding: Finding(
              kind: 'email',
              location: FindingLocation.query,
              spanStart: 3,
              spanEnd: 14,
              tier: FindingTier.regex,
            ),
            location: DraftLocation.query,
            start: 3,
            end: 14,
            valueHash: 'mail',
            value: 'user%40x.de',
          ),
        ],
      );

      final Draft after = reanchorFindings(
        draft.copyWith(pathAndQuery: '/v1/chat?to=user@x.de'),
      );

      expect(openFindings(after), 1);
    });

    test('replace all takes every copy of one value', () {
      final Draft draft = draftOf(
        body: 'a@x.de a@x.de a@x.de a@x.de a@x.de',
        findings: <FindingView>[mail(0, 0)],
      );

      final Draft after = replaceAllOpen(draft, PseudonymNaming());

      expect(after.body, isNot(contains('a@x.de')));
      expect(openFindings(after), 0);
    });

    test('replace all of a value takes every copy of it', () {
      final Draft draft = draftOf(
        body: 'a@x.de a@x.de a@x.de',
        findings: <FindingView>[mail(0, 0)],
      );

      final Draft after = replaceAllOfValue(draft, 'mail', '<EMAIL_1>');

      expect(after.body, '<EMAIL_1> <EMAIL_1> <EMAIL_1>');
    });

    test('an ambiguous finding is removed only when no candidate goes out', () {
      final Draft draft = buildDraft(
        const FlowId('01930000-0000-7000-8000-00000000000f'),
        DraftSource(
          request: HttpRequest(
            method: Method.post,
            scheme: Scheme.https,
            authority: const Authority(host: 'api.example.com', port: 443),
            pathAndQuery: '/v1/chat',
            headers: <Header>[
              Header(name: 'Via', value: 'proxy.one.de'.codeUnits),
              Header(name: 'Via', value: 'mail a@x.de'.codeUnits),
            ],
            body: const BodyRef(sha256: <int>[], size: 0),
          ),
          findings: <Finding>[
            Finding(
              kind: 'email',
              location: FindingLocation.header,
              headerName: 'Via',
              spanStart: 5,
              spanEnd: 11,
              tier: FindingTier.checksum,
            ),
          ],
          bodyText: '',
          bodyKind: BodyKind.empty,
        ),
      );
      Draft withRows(String first, String second) => draft.copyWith(
        headers: <HeaderEntry>[
          draft.headers[0].copyWith(value: first),
          draft.headers[1].copyWith(value: second),
        ],
      );

      // Nur die erste Zeile verliert ihren Kandidaten; die Adresse steht noch.
      expect(
        openFindings(reanchorFindings(withRows('proxy', 'mail a@x.de'))),
        1,
      );
      // Beide weg: nichts davon geht mehr hinaus.
      expect(openFindings(reanchorFindings(withRows('proxy', 'mail'))), 0);
    });
  });

  group('anchoring, minors after round 5 (HUM-161)', () {
    FindingView queryMail() => FindingView(
      index: 0,
      finding: Finding(
        kind: 'email',
        location: FindingLocation.query,
        spanStart: 3,
        spanEnd: 14,
        tier: FindingTier.regex,
      ),
      location: DraftLocation.query,
      start: 3,
      end: 14,
      valueHash: 'mail',
      value: 'user%40x.de',
    );

    test('a stray percent sign does not hide a written-out value', () {
      final Draft draft = draftOf(
        pathAndQuery: '/v1/chat?to=user%40x.de&off=50%',
        findings: <FindingView>[queryMail()],
      );

      final Draft after = reanchorFindings(
        draft.copyWith(pathAndQuery: '/v1/chat?to=user@x.de&off=50%'),
      );

      expect(openFindings(after), 1);
    });

    test('a written-out value that comes back through undo is open again', () {
      final Draft draft = draftOf(
        pathAndQuery: '/v1/chat?to=user%40x.de',
        findings: <FindingView>[queryMail()],
      );
      final Draft written = reanchorFindings(
        draft.copyWith(pathAndQuery: '/v1/chat?to=user@x.de'),
      );
      final Draft gone = reanchorFindings(
        written.copyWith(pathAndQuery: '/v1/chat?to='),
      );
      expect(openFindings(gone), 0);

      final Draft back = reanchorFindings(
        gone.copyWith(pathAndQuery: '/v1/chat?to=user@x.de'),
      );

      expect(openFindings(back), 1);
    });

    test('a manual selection does not reopen at another place', () {
      final Draft draft = draftOf(
        body: 'acme writes',
        headers: const <HeaderEntry>[HeaderEntry(name: 'X-Org', value: 'acme')],
      );

      final Draft after = replaceSelection(
        draft,
        DraftLocation.body,
        0,
        4,
        'client',
        PseudonymNaming(),
      );

      expect(after.body, '<CLIENT_1> writes');
      expect(openFindings(after), 0);
    });
  });

  group('replaceAllOfValue', () {
    test('replaceAllOfValue_sameHashSamePseudonym', () {
      const String value = 'a@x.de';
      final Draft before = draftOf(
        body: 'mail $value',
        headers: <HeaderEntry>[
          const HeaderEntry(name: 'x-user', value: 'a@x.de'),
        ],
        findings: <FindingView>[
          bodyFinding(0, 'email', 5, 11, hash: 'same'),
          headerFinding(1, 'email', 'x-user', 0, 6, hash: 'same'),
        ],
      );

      final Draft after = replaceAllOfValue(before, 'same', '<EMAIL_1>');

      expect(after.body, 'mail <EMAIL_1>');
      expect(after.headers.first.value, '<EMAIL_1>');
      expect(openFindings(after), 0);
      expect(after.pseudonyms['same'], '<EMAIL_1>');
    });
  });

  group('checkHeaders', () {
    HeaderEntry free(String name, String value) =>
        HeaderEntry(name: name, value: value);

    test('a clean set of free rows passes', () {
      expect(
        checkHeaders(<HeaderEntry>[
          free('Content-Type', 'application/json; charset=utf-8'),
          free('X-Note', 'Grüße ä'),
          free("x!#\$%&'*+-.^_`|~9", 'tab\there'),
        ]),
        isNull,
      );
    });

    test('a name with a space or a colon is not a token', () {
      expect(checkHeaders(<HeaderEntry>[free('Bad Name', 'x')]), (
        row: 0,
        problem: HeaderProblem.name,
      ));
      expect(
        checkHeaders(<HeaderEntry>[free('X-A:', 'x')])?.problem,
        HeaderProblem.name,
      );
      // Ein Wert ohne Namen ist keine leere Zeile, sondern eine kaputte.
      expect(
        checkHeaders(<HeaderEntry>[free('', 'x')])?.problem,
        HeaderProblem.name,
      );
    });

    test('a row the daemon sets itself is refused, whatever its case', () {
      for (final String name in <String>[
        'Host',
        'content-length',
        'Transfer-Encoding',
        'Connection',
        'Proxy-Authorization',
      ]) {
        expect(checkHeaders(<HeaderEntry>[free(name, 'x')]), (
          row: 0,
          problem: HeaderProblem.ownedByDaemon,
        ), reason: name);
      }
    });

    test('a control character in a value is refused', () {
      for (final String value in <String>[
        'a\rb',
        'a\nb',
        'a\u0000b',
        'a\u007fb',
      ]) {
        expect(
          checkHeaders(<HeaderEntry>[free('X-A', value)])?.problem,
          HeaderProblem.value,
          reason: value.codeUnits.toString(),
        );
      }
    });

    test('locked rows and empty rows are not checked', () {
      expect(
        checkHeaders(<HeaderEntry>[
          const HeaderEntry(name: 'Host', value: 'a', locked: true),
          free('', ''),
          free('X-B', 'ok'),
        ]),
        isNull,
      );
    });

    test('the first bad row is the one named', () {
      expect(
        checkHeaders(<HeaderEntry>[
          free('X-Ok', 'fine'),
          free('Host', 'evil.io'),
          free('Bad Name', 'x'),
        ])?.row,
        1,
      );
    });
  });

  group('the locked header names', () {
    // Die Liste in Dart ist eine Abschrift; die Wahrheit steht im Daemon. Ein
    // Name, den er setzt oder streicht und der hier fehlt, liesse eine freie
    // Zeile zu, die nie beim Ziel ankommt, ohne dass es jemand sieht.
    Set<String> daemonNames() => <String>{
      ..._rustList(
        '../daemon/crates/proxy/src/edit.rs',
        'DAEMON_OWNED_HEADERS',
      ),
      ..._rustList('../daemon/crates/proxy/src/upstream.rs', 'HOP_BY_HOP'),
    };

    test('both daemon lists are read, not assumed', () {
      expect(daemonNames(), containsAll(<String>['host', 'keep-alive', 'te']));
    });

    test('every name the daemon sets or strips is locked', () {
      for (final String name in daemonNames()) {
        expect(isLockedHeader(name), isTrue, reason: name);
        expect(isLockedHeader(name.toUpperCase()), isTrue, reason: name);
      }
    });

    test('no name is locked that the daemon lets through', () {
      // `proxy-*` sperrt das Präfix, nicht die Liste; es ist bewusst weiter.
      final Set<String> daemon = daemonNames();
      for (final String name in lockedHeaderNames) {
        expect(daemon, contains(name));
      }
    });
  });

  group('buildDraft', () {
    Finding headerFind(String name, int start, int end) => Finding(
      kind: 'email',
      location: FindingLocation.header,
      headerName: name,
      spanStart: start,
      spanEnd: end,
      tier: FindingTier.regex,
    );

    HttpRequest requestWith(List<Header> headers) => HttpRequest(
      method: Method.post,
      scheme: Scheme.https,
      authority: const Authority(host: 'api.example.com', port: 443),
      pathAndQuery: '/v1/chat',
      headers: headers,
      body: const BodyRef(sha256: <int>[], size: 0),
    );

    test('a body span past the text leaves the finding open and unplaced', () {
      // Der Rumpf liegt als Bytes vor, der Text ist kürzer: Der Bereich
      // `[5, 11)` hat keinen Wert, an dem er sich verankern ließe.
      final Draft draft = buildDraft(
        const FlowId('01930000-0000-7000-8000-00000000000f'),
        DraftSource(
          request: requestWith(const <Header>[]),
          findings: <Finding>[
            Finding(
              kind: 'email',
              location: FindingLocation.body,
              spanStart: 5,
              spanEnd: 11,
              tier: FindingTier.regex,
            ),
          ],
          bodyText: 'mail',
          bodyKind: BodyKind.text,
          bodyBytes: Uint8List.fromList(utf8.encode('mail a@x.de')),
        ),
      );

      final Draft typed = reanchorFindings(draft.copyWith(body: 'hello world'));
      final Draft after = replaceAllOpen(typed, PseudonymNaming());

      expect(after.body, 'hello world');
      expect(openFindings(after), 1);
    });

    test('an ambiguous header row leaves the finding open and unplaced', () {
      // Beide `Via` fassen `[5, 11)` und tragen dort Verschiedenes; welche
      // der Daemon meinte, lässt sich ohne den Hash nicht sagen.
      final Draft draft = buildDraft(
        const FlowId('01930000-0000-7000-8000-00000000000f'),
        DraftSource(
          request: requestWith(<Header>[
            Header(name: 'Via', value: 'proxy.one.de'.codeUnits),
            Header(name: 'Via', value: 'mail a@x.de'.codeUnits),
          ]),
          findings: <Finding>[headerFind('Via', 5, 11)],
          bodyText: '',
          bodyKind: BodyKind.empty,
        ),
      );

      final Draft after = replaceAllOpen(draft, PseudonymNaming());

      expect(after.headers[0].value, 'proxy.one.de');
      expect(after.headers[1].value, 'mail a@x.de');
      expect(openFindings(after), 1);
    });

    test('a finding without a place stays open and is never replaced', () {
      // Der Bereich `[20, 26)` endet hinter dem Wert der Kopfzeile; umrechnen
      // lässt er sich nicht. Der Wert geht trotzdem mit hinaus (HUM-161).
      final Draft draft = buildDraft(
        const FlowId('01930000-0000-7000-8000-00000000000f'),
        DraftSource(
          request: requestWith(<Header>[
            Header(name: 'X-Contact', value: 'a@x.de'.codeUnits),
          ]),
          findings: <Finding>[headerFind('X-Contact', 20, 26)],
          bodyText: '',
          bodyKind: BodyKind.empty,
        ),
      );

      expect(draft.findings.single.status, FindingStatus.open);
      expect(openFindings(draft), 1);

      final Draft after = replaceAllOpen(
        reanchorFindings(draft),
        PseudonymNaming(),
      );

      expect(after.headers.single.value, 'a@x.de');
      expect(openFindings(after), 1);
    });

    test('a finding lands in the Via that can hold it, not in the first', () {
      // Die erste `Via` ist zu kurz fuer den Bereich `[0, 6)`; der Fund gehoert
      // in die zweite. Der Daemon schickt nur den Namen mit.
      final Draft draft = buildDraft(
        const FlowId('01930000-0000-7000-8000-00000000000f'),
        DraftSource(
          request: requestWith(<Header>[
            Header(name: 'Via', value: 'ab'.codeUnits),
            Header(name: 'Via', value: 'a@x.de'.codeUnits),
          ]),
          findings: <Finding>[headerFind('Via', 0, 6)],
          bodyText: '',
          bodyKind: BodyKind.empty,
        ),
      );

      expect(draft.findings.single.location.headerIndex, 1);
      expect(draft.textAt(draft.findings.single.location), 'a@x.de');

      final Draft after = replaceFinding(draft, 0, '<EMAIL_1>');

      expect(after.headers[0].value, 'ab');
      expect(after.headers[1].value, '<EMAIL_1>');
    });

    test('a header finding behind an umlaut keeps its characters', () {
      final Draft draft = buildDraft(
        const FlowId('01930000-0000-7000-8000-00000000000f'),
        DraftSource(
          request: requestWith(<Header>[
            Header(name: 'X-Note', value: utf8.encode('Grüsse a@x.de')),
          ]),
          // Byte-Bereich: das `ü` kostet zwei Bytes, also 8 statt 7.
          findings: <Finding>[headerFind('X-Note', 8, 14)],
          bodyText: '',
          bodyKind: BodyKind.empty,
        ),
      );

      final FindingView view = draft.findings.single;
      expect(view.start, 7);
      expect(
        draft.textAt(view.location).substring(view.start, view.end),
        'a@x.de',
      );
    });
  });

  group('duplicate headers', () {
    Draft withTwoVia() => draftOf(
      headers: <HeaderEntry>[
        const HeaderEntry(name: 'Via', value: 'a@x.de'),
        const HeaderEntry(name: 'Via', value: 'b@y.de'),
      ],
    );

    test('a replacement in the second Via leaves the first alone', () {
      final Draft before = withTwoVia().copyWith(
        findings: <FindingView>[
          headerFinding(
            0,
            'email',
            'Via',
            0,
            6,
          ).copyWith(location: DraftLocation.header('Via', index: 1)),
        ],
      );

      final Draft after = replaceFinding(before, 0, '<EMAIL_1>');

      expect(after.headers[0].value, 'a@x.de');
      expect(after.headers[1].value, '<EMAIL_1>');
    });

    test('textAt reads the entry the location points at, not the first', () {
      final Draft draft = withTwoVia();

      expect(draft.textAt(DraftLocation.header('Via', index: 1)), 'b@y.de');
      expect(draft.textAt(DraftLocation.header('Via')), 'a@x.de');
    });

    test('a location for a header that is gone writes nothing', () {
      final Draft draft = draftOf();

      expect(draft.withTextAt(DraftLocation.header('Via'), 'x'), draft);
      expect(draft.textAt(DraftLocation.header('Via')), '');
    });

    test('an index that no longer names the header falls back', () {
      // Der Mensch hat eine Zeile geloescht; die Nummer zeigt jetzt auf eine
      // fremde Kopfzeile. Genommen wird dann wieder die erste gleichnamige.
      final Draft draft = draftOf(
        headers: <HeaderEntry>[
          const HeaderEntry(name: 'Via', value: 'only'),
          const HeaderEntry(name: 'X-Other', value: 'nope'),
        ],
      );

      expect(draft.textAt(DraftLocation.header('Via', index: 1)), 'only');
    });
  });

  group('replaceAllOpen', () {
    test('replaceAllOpen_countersPerType', () {
      final Draft before = draftOf(
        body: 'a@x.de b@y.de DE02120300000000202051',
        findings: <FindingView>[
          bodyFinding(0, 'email', 0, 6),
          bodyFinding(1, 'email', 7, 13),
          bodyFinding(2, 'iban', 14, 36),
        ],
      );

      final Draft after = replaceAllOpen(before, PseudonymNaming());

      expect(after.body, '<EMAIL_1> <EMAIL_2> <IBAN_1>');
      expect(openFindings(after), 0);
    });

    test('replaceAllOpen_userTermAlias', () {
      final Draft before = draftOf(
        body: 'Kunde Müller GmbH zahlt',
        findings: <FindingView>[bodyFinding(0, 'user_term:Müller GmbH', 6, 17)],
      );

      final Draft after = replaceAllOpen(
        before,
        PseudonymNaming(
          aliases: const <String, String>{'Müller GmbH': 'Client-A'},
        ),
      );

      expect(after.body, 'Kunde Client-A zahlt');
    });

    test('the same value gets the same pseudonym across locations', () {
      final Draft before = draftOf(
        body: 'a@x.de and a@x.de',
        findings: <FindingView>[
          bodyFinding(0, 'email', 0, 6, hash: 'one'),
          bodyFinding(1, 'email', 11, 17, hash: 'one'),
        ],
      );

      final Draft after = replaceAllOpen(before, PseudonymNaming());

      expect(after.body, '<EMAIL_1> and <EMAIL_1>');
    });

    test('an ignored finding costs no pseudonym number', () {
      // Der erste Fund ist ignoriert. Bekäme er trotzdem einen Namen, hiesse
      // der zweite `<EMAIL_2>`, und niemand faende je einen `<EMAIL_1>`.
      final Draft before = ignoreFinding(
        draftOf(
          body: 'a@x.de b@y.de',
          findings: <FindingView>[
            bodyFinding(0, 'email', 0, 6),
            bodyFinding(1, 'email', 7, 13, hash: 'two'),
          ],
        ),
        0,
      );

      final Draft after = replaceAllOpen(before, PseudonymNaming());

      expect(after.body, 'a@x.de <EMAIL_1>');
      expect(after.counters['EMAIL'], 1);
    });

    test('an ignored finding is not replaced', () {
      final Draft before = ignoreFinding(
        draftOf(
          body: 'a@x.de',
          findings: <FindingView>[bodyFinding(0, 'email', 0, 6)],
        ),
        0,
      );

      expect(replaceAllOpen(before, PseudonymNaming()).body, 'a@x.de');
    });
  });

  group('replaceSelection', () {
    test('replaceSelection_createsCustomFinding', () {
      // "0123456789PROJEKT!" -- die Auswahl [10, 16) ist "PROJEK".
      final Draft before = draftOf(body: '0123456789PROJEKT!');

      final Draft after = replaceSelection(
        before,
        DraftLocation.body,
        10,
        16,
        'PROJECT',
        PseudonymNaming(),
      );

      expect(after.body, '0123456789<PROJECT_1>T!');
      final FindingView made = after.findings.single;
      expect(made.finding.kind, 'custom:PROJECT');
      expect(made.status, FindingStatus.replaced);
      expect(made.pseudonym, '<PROJECT_1>');
      expect(after.replacements.single.original, 'PROJEK');
    });

    test('an empty label becomes CUSTOM', () {
      final Draft after = replaceSelection(
        draftOf(body: 'secret value'),
        DraftLocation.body,
        0,
        6,
        '   ',
        PseudonymNaming(),
      );

      expect(after.body, '<CUSTOM_1> value');
    });

    test('the same selection twice gets the same pseudonym', () {
      final PseudonymNaming naming = PseudonymNaming();
      final Draft once = replaceSelection(
        draftOf(body: 'acme and acme'),
        DraftLocation.body,
        9,
        13,
        'CLIENT',
        naming,
      );
      final Draft twice = replaceSelection(
        once,
        DraftLocation.body,
        0,
        4,
        'CLIENT',
        PseudonymNaming(existing: once.pseudonyms, counters: once.counters),
      );

      expect(twice.body, '<CLIENT_1> and <CLIENT_1>');
    });

    test('a selection outside the text changes nothing', () {
      final Draft before = draftOf(body: 'short');

      expect(
        replaceSelection(
          before,
          DraftLocation.body,
          2,
          99,
          'X',
          PseudonymNaming(),
        ),
        before,
      );
    });
  });

  group('renderBody', () {
    test('renderBody_jsonInvalidAfterEdit', () {
      // Der Fund deckt das Anfuehrungszeichen mit ab; die Ersetzung zerreisst
      // damit den String.
      const String body = '{"a":"x@y.de"}';
      final Draft before = draftOf(
        body: body,
        kind: BodyKind.json,
        findings: <FindingView>[bodyFinding(0, 'email', 6, 13)],
      );
      expect(renderBody(before).jsonError, isNull);

      final Draft after = replaceFinding(before, 0, '<EMAIL_1>');

      // Der Fund deckte das schliessende Anfuehrungszeichen mit ab; nach der
      // Ersetzung ist der String offen.
      expect(after.body, '{"a":"<EMAIL_1>}');
      expect(renderBody(after).jsonError, isNotNull);
    });

    test('a replacement inside the quotes keeps the json valid', () {
      final Draft before = draftOf(
        body: '{"a":"x@y.de"}',
        kind: BodyKind.json,
        findings: <FindingView>[bodyFinding(0, 'email', 6, 12)],
      );

      final Draft after = replaceFinding(before, 0, '<EMAIL_1>');

      expect(after.body, '{"a":"<EMAIL_1>"}');
      expect(renderBody(after).jsonError, isNull);
    });

    test('a body that is not json is never checked', () {
      final Draft before = draftOf(body: 'not json at all');

      expect(renderBody(before).jsonError, isNull);
    });
  });

  group('openFindings', () {
    test('openFindings_excludesIgnoredAndReplaced', () {
      final Draft before = draftOf(
        body: 'a@x.de b@y.de c@z.de',
        findings: <FindingView>[
          bodyFinding(0, 'email', 0, 6),
          bodyFinding(1, 'email', 7, 13),
          bodyFinding(2, 'email', 14, 20),
        ],
      );
      expect(openFindings(before), 3);

      final Draft after = ignoreFinding(
        replaceFinding(before, 0, '<EMAIL_1>'),
        1,
      );

      expect(openFindings(after), 1);
    });
  });

  group('toProto', () {
    test('toProto_dropsLockedHeaders', () {
      final Draft draft = draftOf(
        headers: <HeaderEntry>[
          const HeaderEntry(name: 'Content-Length', value: '17', locked: true),
          const HeaderEntry(
            name: 'Host',
            value: 'api.example.com',
            locked: true,
          ),
          const HeaderEntry(name: 'content-type', value: 'application/json'),
        ],
      );

      final EditedRequest wire = buildEditedRequest(
        method: draft.method,
        url: draft.url,
        headers: <({String name, String value})>[
          for (final HeaderEntry entry in draft.headers)
            (name: entry.name, value: entry.value),
        ],
        body: draft.body,
      );

      expect(
        <String>[for (final Header header in wire.headers) header.name],
        <String>['content-type'],
      );
      expect(wire.url, 'https://api.example.com/v1/chat');
      expect(wire.method, Method.post);
    });

    test('the body travels as utf-8 bytes, not as code units', () {
      // "Grüsse" ist 6 Zeichen und 7 Bytes; `content-length` zaehlt Bytes.
      final EditedRequest wire = buildEditedRequest(
        method: 'POST',
        url: 'https://api.example.com/v1/chat',
        headers: const <({String name, String value})>[],
        body: 'Grüsse',
      );

      expect('Grüsse'.length, 6);
      expect(wire.body.length, 7);
    });

    test('an unknown method travels as OTHER with its raw token', () {
      final EditedRequest wire = buildEditedRequest(
        method: 'PURGE',
        url: 'https://api.example.com/x',
        headers: const <({String name, String value})>[],
        body: '',
      );

      expect(wire.method, Method.other);
      expect(wire.methodRaw, 'PURGE');
    });
  });

  group('checkEditedRequest', () {
    test('a lowercase method is refused, like EDIT_002 would', () {
      expect(
        checkEditedRequest(method: 'post', pathAndQuery: '/x'),
        EditedRequestProblem.method,
      );
    });

    test('a method of seventeen letters is refused', () {
      expect(
        checkEditedRequest(method: 'A' * 17, pathAndQuery: '/x'),
        EditedRequestProblem.method,
      );
      expect(checkEditedRequest(method: 'A' * 16, pathAndQuery: '/x'), isNull);
    });

    test('a path without a leading slash is refused, like EDIT_003 would', () {
      expect(
        checkEditedRequest(method: 'GET', pathAndQuery: 'x'),
        EditedRequestProblem.path,
      );
    });

    test('a path with a space is refused', () {
      expect(
        checkEditedRequest(method: 'GET', pathAndQuery: '/a b'),
        EditedRequestProblem.path,
      );
    });

    test('an unencoded umlaut in the path is refused', () {
      expect(
        checkEditedRequest(method: 'GET', pathAndQuery: '/grüsse'),
        EditedRequestProblem.path,
      );
      expect(
        checkEditedRequest(method: 'GET', pathAndQuery: '/gr%C3%BCsse'),
        isNull,
      );
    });
  });

  group('PseudonymNaming', () {
    test('typeLabel drops the parameter of a kind', () {
      expect(PseudonymNaming.typeLabel('api_key:github'), 'API_KEY');
      expect(PseudonymNaming.typeLabel('email'), 'EMAIL');
      expect(PseudonymNaming.typeLabel('credit_card'), 'CARD');
      expect(PseudonymNaming.typeLabel('something_new'), 'CUSTOM');
      // `custom:` behaelt das Wort des Menschen: sonst haette `Ctrl+R` mit
      // einem Label dieselbe Wirkung wie ohne.
      expect(PseudonymNaming.typeLabel('custom:PROJECT'), 'PROJECT');
      expect(PseudonymNaming.typeLabel('custom:'), 'CUSTOM');
    });

    test('labelFrom keeps letters and digits and nothing else', () {
      expect(PseudonymNaming.labelFrom('project'), 'PROJECT');
      expect(PseudonymNaming.labelFrom('ACME GmbH'), 'ACME_GMBH');
      expect(PseudonymNaming.labelFrom('kunde-2'), 'KUNDE_2');
      expect(PseudonymNaming.labelFrom('---'), 'CUSTOM');
    });

    test('a value without a hash gets a name but no entry', () {
      final PseudonymNaming naming = PseudonymNaming();

      expect(naming.nameFor('email', ''), '<EMAIL_1>');
      expect(naming.nameFor('email', ''), '<EMAIL_2>');
      expect(naming.assigned, isEmpty);
    });

    test('counters carry over from a previous instance', () {
      final PseudonymNaming first = PseudonymNaming();
      first.nameFor('email', 'a');

      final PseudonymNaming second = PseudonymNaming(
        existing: first.assigned,
        counters: first.counters,
      );

      expect(second.nameFor('email', 'b'), '<EMAIL_2>');
      expect(second.nameFor('email', 'a'), '<EMAIL_1>');
    });
  });

  group('maskedOriginal', () {
    test('it keeps two characters at each end', () {
      const Replacement done = Replacement(
        location: DraftLocation.body,
        start: 0,
        end: 9,
        original: 'anna@example.com',
        pseudonym: '<EMAIL_1>',
      );

      expect(done.maskedOriginal, 'an************om');
    });

    test('a short value is masked whole', () {
      const Replacement done = Replacement(
        location: DraftLocation.body,
        start: 0,
        end: 1,
        original: 'ab',
        pseudonym: '<X_1>',
      );

      expect(done.maskedOriginal, '**');
    });
  });
}

/// Die Zeichenketten der Rust-Konstante [name] in der Datei [path].
///
/// Liest den Quelltext als Text, wie `history_meta_test.dart` es für den
/// Filter des Recorders tut: vom Namen bis zur schliessenden Klammer `];`.
Set<String> _rustList(String path, String name) {
  final String source = File(path).readAsStringSync();
  final int start = source.indexOf('const $name');
  expect(start, isNot(-1), reason: '$name in $path');
  final int end = source.indexOf('];', start);
  return RegExp(r'"([^"]+)"')
      .allMatches(source.substring(start, end))
      .map((RegExpMatch match) => match.group(1)!)
      .toSet();
}
