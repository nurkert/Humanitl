// Was die vier Ansichten zeichnen, und was sie nicht zeichnen dürfen.
//
// Ein Fund, der in einer Ansicht unsichtbar bleibt, ist gefährlicher als eine
// fehlende Ansicht; deshalb prüft diese Datei für jede Ansicht, dass kein Fund
// verschwindet, und für den Rumpf selbst, dass er nichts anfassbares und
// nichts richtungsdrehendes auf den Schirm bringt.

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/body/body_decode.dart';
import 'package:humanitl/features/intercept/body/body_kind.dart';
import 'package:humanitl/features/intercept/body/body_marks.dart';
import 'package:humanitl/features/intercept/body/body_parser.dart';
import 'package:humanitl/features/intercept/body/body_span.dart';
import 'package:humanitl/features/intercept/body/body_view.dart';
import 'package:humanitl/features/intercept/body/form_view.dart';
import 'package:humanitl/features/intercept/body/hex_view.dart';
import 'package:humanitl/features/intercept/body/json_tree_view.dart';
import 'package:humanitl/features/intercept/body/raw_view.dart';

import 'harness.dart';

/// Ein JSON-Rumpf mit drei Funden an bekannten Stellen.
({ParsedBody parsed, Uint8List bytes}) jsonWithFindings() {
  const String source =
      '{"token":"ghp_A1B2C3D4","mail":"a@b.de","host":"10.0.0.7"}';
  final Uint8List bytes = bytesOf(source);
  List<Finding> at(String needle, String kind, FindingTier tier) {
    final int start = source.indexOf(needle);
    return <Finding>[
      bodyFinding(
        start: start,
        end: start + needle.length,
        kind: kind,
        tier: tier,
      ),
    ];
  }

  return (
    parsed: parseBody(bytes, BodyKind.json, <Finding>[
      ...at('ghp_A1B2C3D4', 'api_key:github', FindingTier.checksum),
      ...at('a@b.de', 'email', FindingTier.regex),
      ...at('10.0.0.7', 'ipv4', FindingTier.regex),
    ]),
    bytes: bytes,
  );
}

void main() {
  group('no finding disappears when the view changes', () {
    test('json body: tree, raw and hex mark all three', () {
      final ParsedBody parsed = jsonWithFindings().parsed;
      expect(parsed.findings, hasLength(3));
      for (final BodyPane pane in <BodyPane>[
        BodyPane.tree,
        BodyPane.raw,
        BodyPane.hex,
      ]) {
        expect(
          unmarkedFindings(parsed, pane, parsed.bytes.length),
          isEmpty,
          reason: '$pane',
        );
      }
    });

    test('form body: form, raw and hex mark the name and the value', () {
      const String source = 'a%40b.de=1&token=ghp_A1B2C3D4';
      final int mail = source.indexOf('a%40b.de');
      final int token = source.indexOf('ghp_A1B2C3D4');
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.form,
        <Finding>[
          bodyFinding(start: mail, end: mail + 8, kind: 'email'),
          bodyFinding(
            start: token,
            end: token + 12,
            kind: 'api_key:github',
            tier: FindingTier.checksum,
          ),
        ],
      );
      for (final BodyPane pane in <BodyPane>[
        BodyPane.form,
        BodyPane.raw,
        BodyPane.hex,
      ]) {
        expect(
          unmarkedFindings(parsed, pane, parsed.bytes.length),
          isEmpty,
          reason: '$pane',
        );
      }
    });

    test('a finding the tree cannot place is named, not dropped', () {
      // Der Wert steht in der Quelle escaped; im Baum steht er dekodiert, also
      // findet die Textsuche ihn nicht. Genau dieser Fall darf nicht still
      // bleiben.
      const String source = r'{"mail":"a\u0040b.de"}';
      const String escaped = r'a\u0040b.de';
      final int start = source.indexOf(escaped);
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.json,
        <Finding>[bodyFinding(start: start, end: start + escaped.length)],
      );
      // Im Baum steht der dekodierte Wert; der Treffertext der Quelle kommt
      // darin nicht vor, und genau deshalb muss die Ansicht ihn nennen.
      expect(parsed.json!.nodes.last.full, 'a@b.de');
      expect(parsed.json!.unlocatedFindings, <int>{0});
      expect(
        unmarkedFindings(parsed, BodyPane.tree, parsed.bytes.length),
        <int>{0},
      );
      expect(
        bodyNotes(parsed, BodyPane.tree, source.length, english),
        contains(english.interceptBodyFindingsElsewhere(1)),
      );
      // In der Rohansicht sitzt er über seinem Versatz.
      expect(
        unmarkedFindings(parsed, BodyPane.raw, parsed.bytes.length),
        isEmpty,
      );
    });
  });

  group('what the notes say', () {
    test('a lying content type is named, over the whole way', () {
      // Nicht `disputedType: true` einsetzen: geprüft wird der Weg von den
      // Bytes des Transports über `buildBodyLoad` bis in den Satz. Sonst
      // ließe sich die Zuweisung löschen, ohne dass etwas rot wird.
      final Uint8List png = Uint8List.fromList(<int>[
        0x89,
        0x50,
        0x4E,
        0x47,
        0x0D,
        0x0A,
        0x1A,
        0x0A,
        ...List<int>.generate(300, (int i) => (i * 7) % 256),
      ]);
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: png),
        BodyRef(
          sha256: List<int>.filled(32, 3),
          size: png.length,
          contentType: 'application/json',
        ),
        '',
      );
      expect(load.kind, BodyKind.binary);
      expect(load.disputedType, isTrue);
      final ParsedBody parsed = parseLoadedBody(load, const <Finding>[]);
      expect(
        bodyNotes(parsed, BodyPane.hex, load.bytes.length, english),
        contains(english.interceptBodyTypeDisputed),
      );
    });

    test('a finding past the row cap is named, not silently dropped', () {
      // Die Zeilen enden bei `bodyMaxRows`, der Text nicht. Vorher zählte ein
      // Fund dahinter als sichtbar, bekam keinen Unterstrich, und der Sprung
      // landete auf der letzten gebauten Zeile.
      final String many = List<String>.filled(bodyMaxRows + 10, 'a').join('\n');
      final String source = '$many\nghp_A1B2C3D4';
      final int token = source.lastIndexOf('ghp_A1B2C3D4');
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.text,
        <Finding>[
          bodyFinding(
            start: token,
            end: token + 12,
            kind: 'api_key:github',
            tier: FindingTier.checksum,
          ),
        ],
      );
      final BodyText text = parsed.text!;
      expect(text.rowsCapped, isTrue);
      expect(text.rows.length, bodyMaxRows);
      expect(
        parsed.findings.single.charStart,
        greaterThan(text.rows.last.charEnd),
      );
      expect(findingsByRow(text.rows, parsed.findings), isEmpty);
      expect(rowOfChar(text.rows, parsed.findings.single.charStart), isNull);
      expect(unmarkedFindings(parsed, BodyPane.raw, parsed.bytes.length), <int>{
        0,
      });
      expect(
        bodyNotes(parsed, BodyPane.raw, parsed.bytes.length, english),
        contains(english.interceptBodyFindingsElsewhere(1)),
      );
    });

    test('a finding past the loaded bytes is named in hex too', () {
      // Bei einem zu großen Rumpf werden nur die ersten 64 KiB gezeigt; ein
      // Fund dahinter hat dort keine Zelle.
      final ParsedBody parsed = parseBody(
        bytesOf('short'),
        BodyKind.text,
        <Finding>[bodyFinding(start: 900000, end: 900006)],
      );
      expect(unmarkedFindings(parsed, BodyPane.hex, 5), <int>{0});
    });

    test('an incomplete body never reads as an empty one', () {
      final ParsedBody parsed = parseBody(
        bytesOf('half of it'),
        BodyKind.text,
        const <Finding>[],
        problem: BodyProblem.incomplete,
      );
      final List<String> notes = bodyNotes(parsed, BodyPane.raw, 10, english);
      expect(notes, contains(english.interceptBodyIncomplete));
      expect(notes, isNot(contains(english.interceptBodyEmpty)));
    });

    test('hex says it stops at 64 KB only when it does', () {
      final ParsedBody parsed = parseBody(
        bytesOf('short'),
        BodyKind.text,
        const <Finding>[],
      );
      expect(
        bodyNotes(parsed, BodyPane.hex, 5, english),
        isNot(contains(english.interceptBodyHexTruncated)),
      );
      expect(
        bodyNotes(parsed, BodyPane.hex, bodyHexLimit + 1, english),
        contains(english.interceptBodyHexTruncated),
      );
    });

    test('duplicate keys are said out loud', () {
      final ParsedBody parsed = parseBody(
        bytesOf('{"amount":1,"amount":999}'),
        BodyKind.json,
        const <Finding>[],
      );
      expect(
        bodyNotes(parsed, BodyPane.tree, 25, english),
        contains(english.interceptBodyDuplicateKeys),
      );
    });
  });

  group('the raw view', () {
    testWidgets('marks exactly the match and nothing beside it', (
      WidgetTester tester,
    ) async {
      final ParsedBody parsed = jsonWithFindings().parsed;
      await pumpBody(
        tester,
        RawBodyView(text: parsed.text!, findings: parsed.findings),
      );
      final List<String> underlined = <String>[
        for (final TextSpan span in spansOf(tester, find.byType(RichText)))
          if (span.style?.decoration == TextDecoration.underline)
            span.text ?? '',
      ];
      expect(underlined, <String>['ghp_A1B2C3D4', 'a@b.de', '10.0.0.7']);
    });

    testWidgets('carries no tap target and no link', (
      WidgetTester tester,
    ) async {
      // Ein Rumpf ist Inhalt. Ein Span mit Erkenner wäre der erste Schritt zu
      // einer Oberfläche, die der Absender mitgestaltet.
      final ParsedBody parsed = parseBody(
        bytesOf('see https://evil.example/ and [click](x)'),
        BodyKind.text,
        const <Finding>[],
      );
      await pumpBody(
        tester,
        RawBodyView(text: parsed.text!, findings: parsed.findings),
      );
      for (final TextSpan span in spansOf(tester, find.byType(RichText))) {
        expect(span.recognizer, isNull);
      }
      expect(find.byType(GestureDetector), findsNothing);
    });

    testWidgets('draws no chroma except the findings', (
      WidgetTester tester,
    ) async {
      final ParsedBody parsed = jsonWithFindings().parsed;
      await pumpBody(
        tester,
        RawBodyView(text: parsed.text!, findings: parsed.findings),
      );
      final HTokens tokens = HTokens.dark;
      final Set<Color> ladder = <Color>{
        tokens.colors.fg0,
        tokens.colors.fg1,
        tokens.colors.fg2,
      };
      for (final TextSpan span in spansOf(tester, find.byType(RichText))) {
        final Color? color = span.style?.color;
        if (color != null) {
          expect(ladder, contains(color));
        }
      }
    });

    testWidgets('a right-to-left override never reaches the screen', (
      WidgetTester tester,
    ) async {
      const String hostile = 'amount: 123\u{202E}456 EUR';
      final ParsedBody parsed = parseBody(
        bytesOf(hostile),
        BodyKind.text,
        const <Finding>[],
      );
      await pumpBody(
        tester,
        RawBodyView(text: parsed.text!, findings: parsed.findings),
      );
      for (final TextSpan span in spansOf(tester, find.byType(RichText))) {
        expect(span.text?.contains('\u{202E}') ?? false, isFalse);
      }
      for (final RichText text in tester.widgetList<RichText>(
        find.byType(RichText),
      )) {
        expect(text.textDirection, TextDirection.ltr);
      }
    });
  });

  group('the hex view', () {
    testWidgets('hex_first_64k_only', (WidgetTester tester) async {
      final Uint8List big = Uint8List(bodyHexLimit + 4096);
      await pumpBody(
        tester,
        HexView(
          bytes: big,
          findings: const <BodyFinding>[],
          limit: bodyHexLimit,
        ),
      );
      final ListView list = tester.widget<ListView>(find.byType(ListView));
      expect(list.semanticChildCount, bodyHexLimit ~/ hexBytesPerRow);
    });

    testWidgets('shows a finding in both columns', (WidgetTester tester) async {
      final ({ParsedBody parsed, Uint8List bytes}) body = jsonWithFindings();
      await pumpBody(
        tester,
        HexView(
          bytes: body.bytes,
          findings: body.parsed.findings,
          limit: bodyHexLimit,
        ),
      );
      final int marked = spansOf(tester, find.byType(RichText))
          .where(
            (TextSpan span) =>
                span.style?.decoration == TextDecoration.underline,
          )
          .length;
      // Jedes markierte Byte bekommt eine Zelle in Hex und eine in ASCII.
      final int bytesFound = body.parsed.findings.fold<int>(
        0,
        (int sum, BodyFinding f) => sum + (f.byteEnd - f.byteStart),
      );
      expect(marked, bytesFound * 2);
    });
  });

  group('the tree', () {
    testWidgets('opens only the children of the node that was tapped', (
      WidgetTester tester,
    ) async {
      final String source = jsonEncode(<String, Object?>{
        'a': <String, Object?>{for (int i = 0; i < 12; i++) 'x$i': i},
        'b': <String, Object?>{for (int i = 0; i < 12; i++) 'y$i': i},
      });
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.json,
        const <Finding>[],
      );
      await pumpBody(
        tester,
        JsonTreeView(document: parsed.json!, findings: parsed.findings),
        size: const Size(800, 1200),
      );
      ListView list() => tester.widget<ListView>(find.byType(ListView));
      expect(list().semanticChildCount, 3);
      await tester.tap(find.byType(GestureDetector).at(1));
      await tester.pump();
      expect(list().semanticChildCount, 3 + 12);
    });

    testWidgets('marks the value and the path to it', (
      WidgetTester tester,
    ) async {
      final ParsedBody parsed = jsonWithFindings().parsed;
      expect(parsed.json!.markedAncestors, contains(0));
      await pumpBody(
        tester,
        JsonTreeView(document: parsed.json!, findings: parsed.findings),
      );
      final int underlined = spansOf(tester, find.byType(RichText))
          .where(
            (TextSpan span) =>
                span.style?.decoration == TextDecoration.underline,
          )
          .length;
      expect(underlined, 3);
    });
  });

  group('the form view', () {
    testWidgets('shows the decoded value and marks the finding in it', (
      WidgetTester tester,
    ) async {
      const String source = 'to=a%40b.de&note=x+y';
      final int mail = source.indexOf('a%40b.de');
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.form,
        <Finding>[bodyFinding(start: mail, end: mail + 8)],
      );
      await pumpBody(
        tester,
        FormView(pairs: parsed.form!, findings: parsed.findings),
      );
      final List<String> texts = <String>[
        for (final TextSpan span in spansOf(tester, find.byType(RichText)))
          span.text ?? '',
      ];
      expect(texts, contains('a@b.de'));
      expect(texts, contains('x y'));
      final List<String> underlined = <String>[
        for (final TextSpan span in spansOf(tester, find.byType(RichText)))
          if (span.style?.decoration == TextDecoration.underline)
            span.text ?? '',
      ];
      expect(underlined, <String>['a@b.de']);
    });
  });

  group('what a column cannot draw is not called visible', () {
    test('a finding past the form name column is named', () {
      // Die Namensspalte hört bei `formNameChars` auf. Ein Fund dahinter
      // bekommt keine Markierung, also darf er auch nicht als verortet
      // zählen -- sonst schwiege die Ansicht über ihn.
      final String name = 'x' * (formNameChars + 40);
      final String source = '$name=1';
      final int late = formNameChars + 10;
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.form,
        <Finding>[bodyFinding(start: late, end: late + 6)],
      );
      expect(parsed.form!.single.name.length, name.length);
      expect(formLocatedFindings(parsed.form!, parsed.findings), isEmpty);
      expect(
        unmarkedFindings(parsed, BodyPane.form, parsed.bytes.length),
        <int>{0},
      );
      expect(
        bodyNotes(parsed, BodyPane.form, parsed.bytes.length, english),
        contains(english.interceptBodyFindingsElsewhere(1)),
      );
    });

    test('a finding inside the drawn part of a long name is marked', () {
      final String name = 'x' * (formNameChars + 40);
      final ParsedBody parsed = parseBody(
        bytesOf('$name=1'),
        BodyKind.form,
        <Finding>[bodyFinding(start: 4, end: 10)],
      );
      expect(formLocatedFindings(parsed.form!, parsed.findings), <int>{0});
    });
  });

  group('the underline says exactly where', () {
    testWidgets('a secret inside an address is not swallowed by it', (
      WidgetTester tester,
    ) async {
      // Der strengere Ton gewinnt über den milderen, und der mildere bleibt
      // links und rechts davon stehen. Vorher verschwand der innere Fund, und
      // der Mensch sah die harmlosere Einstufung.
      const String source = '{"user":"ghp_A1B2C3D4@corp.example"}';
      final int outer = source.indexOf('ghp_A1B2C3D4@corp.example');
      final int inner = source.indexOf('ghp_A1B2C3D4');
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.json,
        <Finding>[
          bodyFinding(start: outer, end: outer + 25, kind: 'email'),
          bodyFinding(
            start: inner,
            end: inner + 12,
            kind: 'api_key:github',
            tier: FindingTier.checksum,
          ),
        ],
      );
      await pumpBody(
        tester,
        RawBodyView(text: parsed.text!, findings: parsed.findings),
      );
      final HTokens tokens = HTokens.dark;
      final Map<Color, String> byColor = <Color, String>{};
      for (final TextSpan span in spansOf(tester, find.byType(RichText))) {
        if (span.style?.decoration != TextDecoration.underline) {
          continue;
        }
        byColor[span.style!.decorationColor!] =
            (byColor[span.style!.decorationColor!] ?? '') + (span.text ?? '');
      }
      expect(
        byColor[bodyFindingColor(tokens, BodyFindingTone.secret)],
        'ghp_A1B2C3D4',
      );
      expect(
        byColor[bodyFindingColor(tokens, BodyFindingTone.personal)],
        '@corp.example',
      );
    });

    testWidgets('the tree underlines the match, not the whole value', (
      WidgetTester tester,
    ) async {
      const String source =
          '{"note":"harmless words around ghp_A1B2C3D4 and more words"}';
      final int at = source.indexOf('ghp_A1B2C3D4');
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.json,
        <Finding>[
          bodyFinding(
            start: at,
            end: at + 12,
            kind: 'api_key:github',
            tier: FindingTier.checksum,
          ),
        ],
      );
      await pumpBody(
        tester,
        JsonTreeView(document: parsed.json!, findings: parsed.findings),
      );
      final List<String> underlined = <String>[
        for (final TextSpan span in spansOf(tester, find.byType(RichText)))
          if (span.style?.decoration == TextDecoration.underline)
            span.text ?? '',
      ];
      expect(underlined, <String>['ghp_A1B2C3D4']);
    });

    testWidgets('a finding in a key underlines the key, not the value', (
      WidgetTester tester,
    ) async {
      const String source = '{"ghp_A1B2C3D4":"harmless"}';
      final int at = source.indexOf('ghp_A1B2C3D4');
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.json,
        <Finding>[
          bodyFinding(
            start: at,
            end: at + 12,
            kind: 'api_key:github',
            tier: FindingTier.checksum,
          ),
        ],
      );
      expect(parsed.json!.findingsByNode.values.single.single.inKey, isTrue);
      await pumpBody(
        tester,
        JsonTreeView(document: parsed.json!, findings: parsed.findings),
      );
      final List<String> underlined = <String>[
        for (final TextSpan span in spansOf(tester, find.byType(RichText)))
          if (span.style?.decoration == TextDecoration.underline)
            span.text ?? '',
      ];
      expect(underlined, <String>['ghp_A1B2C3D4']);
    });

    test('a finding behind the shortened value is not underlined at all', () {
      // Der Baum kürzt bei `jsonValueChars`. Ein Fund dahinter darf nicht den
      // sichtbaren Anfang unterstreichen; er gilt als nicht verortet.
      final String long = '${'x' * (jsonValueChars + 50)}ghp_A1B2C3D4';
      final String source = '{"note":"$long"}';
      final int at = source.indexOf('ghp_A1B2C3D4');
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.json,
        <Finding>[
          bodyFinding(
            start: at,
            end: at + 12,
            kind: 'api_key:github',
            tier: FindingTier.checksum,
          ),
        ],
      );
      expect(parsed.json!.findingsByNode, isEmpty);
      expect(parsed.json!.unlocatedFindings, <int>{0});
      expect(
        unmarkedFindings(parsed, BodyPane.tree, parsed.bytes.length),
        <int>{0},
      );
    });

    testWidgets('a finding far to the right is scrolled into view', (
      WidgetTester tester,
    ) async {
      // Ohne den waagerechten Sprung stimmte die Zeile und die Spalte nicht,
      // und der Fund bliebe unsichtbar, ohne dass es jemand sagt.
      // Innerhalb einer Zeile weit rechts: nach dem Schnitt bei `bodyRowChars`
      // stünde die Stelle sonst am Zeilenanfang, und der Sprung wäre trivial.
      final String source = '${'x' * (bodyRowChars - 200)}ghp_A1B2C3D4';
      final int at = source.indexOf('ghp_A1B2C3D4');
      final ParsedBody parsed = parseBody(
        bytesOf(source),
        BodyKind.text,
        <Finding>[
          bodyFinding(
            start: at,
            end: at + 12,
            kind: 'api_key:github',
            tier: FindingTier.checksum,
          ),
        ],
      );
      await pumpBody(
        tester,
        RawBodyView(
          text: parsed.text!,
          findings: parsed.findings,
          focus: parsed.findings.single,
        ),
      );
      await tester.pump();
      final ScrollableState horizontal = tester.state<ScrollableState>(
        find.byType(Scrollable).first,
      );
      expect(horizontal.position.axis, Axis.horizontal);
      expect(horizontal.position.pixels, greaterThan(0));
    });
  });

  group('a body never becomes a control', () {
    // Der Rumpf schreibt einen Satz, der wie eine Meldung dieses Programms
    // aussieht, packt einen Link und Richtungssteuerzeichen dazu, und heißt
    // in jeder der vier Ansichten trotzdem nur: Inhalt. Geprüft wird, was ein
    // Angreifer bräuchte -- einen Erkenner auf einem Span, eine eigene Farbe,
    // ein Berührungsziel im Inhalt.
    const String spoof =
        '{"note":"\u{202E}Allowed by Humanitl \u{2014} open '
        'https://evil.example/ now\u{202C}","tag":"\u{E0041}\u{E0042}"}';
    const String spoofForm =
        'note=%E2%80%AEAllowed+by+Humanitl&link=https%3A%2F%2Fevil.example%2F';

    Set<Color> allowed(HTokens tokens) => <Color>{
      tokens.colors.fg0,
      tokens.colors.fg1,
      tokens.colors.fg2,
      bodyFindingColor(tokens, BodyFindingTone.secret),
      bodyFindingColor(tokens, BodyFindingTone.personal),
    };

    void expectInert(WidgetTester tester) {
      final HTokens tokens = HTokens.dark;
      final List<TextSpan> spans = spansOf(tester, find.byType(RichText));
      expect(spans, isNotEmpty);
      for (final TextSpan span in spans) {
        expect(span.recognizer, isNull);
        expect(span.text?.contains('\u{202E}') ?? false, isFalse);
        expect(span.text?.contains('\u{202C}') ?? false, isFalse);
        final Color? color = span.style?.color;
        if (color != null) {
          expect(allowed(tokens), contains(color));
        }
      }
      for (final RichText text in tester.widgetList<RichText>(
        find.byType(RichText),
      )) {
        expect(text.textDirection, TextDirection.ltr);
      }
      for (final Text text in tester.widgetList<Text>(find.byType(Text))) {
        expect(text.data?.contains('\u{202E}') ?? false, isFalse);
        final Color? color = text.style?.color;
        if (color != null) {
          expect(allowed(tokens), contains(color));
        }
      }
    }

    testWidgets('the raw view', (WidgetTester tester) async {
      final ParsedBody parsed = parseBody(
        bytesOf(spoof),
        BodyKind.json,
        const <Finding>[],
      );
      await pumpBody(
        tester,
        RawBodyView(text: parsed.text!, findings: parsed.findings),
      );
      expectInert(tester);
      expect(find.byType(GestureDetector), findsNothing);
    });

    testWidgets('the hex view', (WidgetTester tester) async {
      await pumpBody(
        tester,
        HexView(
          bytes: bytesOf(spoof),
          findings: const <BodyFinding>[],
          limit: bodyHexLimit,
        ),
      );
      expectInert(tester);
      expect(find.byType(GestureDetector), findsNothing);
    });

    testWidgets('the form view', (WidgetTester tester) async {
      final ParsedBody parsed = parseBody(
        bytesOf(spoofForm),
        BodyKind.form,
        const <Finding>[],
      );
      await pumpBody(
        tester,
        FormView(pairs: parsed.form!, findings: parsed.findings),
      );
      expectInert(tester);
      expect(find.byType(GestureDetector), findsNothing);
    });

    testWidgets('the tree opens no tooltip over a shortened value', (
      WidgetTester tester,
    ) async {
      // Ein Kurzhinweis mit fremdem Text in der Schrift und im Rahmen der
      // Anwendung wäre genau der Fall "der Rumpf sieht aus wie unsere
      // Oberfläche". Die Rohansicht zeigt den ganzen Wert, in Monospace.
      final String long = 'y' * (jsonValueChars + 500);
      final ParsedBody parsed = parseBody(
        bytesOf('{"note":"$long"}'),
        BodyKind.json,
        const <Finding>[],
      );
      await pumpBody(
        tester,
        JsonTreeView(document: parsed.json!, findings: parsed.findings),
      );
      expect(find.byType(HoverLabel), findsNothing);
      expect(find.byType(OverlayPortal), findsNothing);
      final TextStyle mono = HTokens.dark.typography.mono13;
      for (final TextSpan span in spansOf(tester, find.byType(RichText))) {
        if (span.text == null || span.text!.isEmpty) {
          continue;
        }
        expect(span.style?.fontFamily, mono.fontFamily);
      }
    });

    testWidgets('the tree, whose only control is the chevron', (
      WidgetTester tester,
    ) async {
      final ParsedBody parsed = parseBody(
        bytesOf(spoof),
        BodyKind.json,
        const <Finding>[],
      );
      await pumpBody(
        tester,
        JsonTreeView(document: parsed.json!, findings: parsed.findings),
      );
      expectInert(tester);
      // Genau ein Aufklapp-Ziel: die Wurzel. Die Werte darunter sind Inhalt
      // und reagieren auf nichts.
      expect(find.byType(GestureDetector), findsOneWidget);
    });

    test('the tag characters of an invisible sentence are replaced', () {
      final ParsedBody parsed = parseBody(
        bytesOf(spoof),
        BodyKind.json,
        const <Finding>[],
      );
      expect(parsed.text!.text.contains('\u{E0041}'), isFalse);
      expect(parsed.text!.text.length, spoof.length);
      for (final JsonNode node in parsed.json!.nodes) {
        expect(node.full.contains('\u{E0041}'), isFalse);
        expect(node.full.contains('\u{202E}'), isFalse);
      }
    });
  });

  test('the two tones are the two the design allows', () {
    final HTokens tokens = HTokens.dark;
    expect(
      bodyFindingColor(tokens, BodyFindingTone.secret),
      tokens.stateColor(HFlowState.error),
    );
    expect(
      bodyFindingColor(tokens, BodyFindingTone.personal),
      tokens.stateColor(HFlowState.held),
    );
  });

  test('a pane the kind does not offer falls back, it does not stick', () {
    expect(effectiveBodyPane(BodyKind.text, BodyPane.tree), BodyPane.raw);
    expect(effectiveBodyPane(BodyKind.json, BodyPane.hex), BodyPane.hex);
    expect(effectiveBodyPane(BodyKind.json, null), BodyPane.tree);
    expect(effectiveBodyPane(BodyKind.binary, null), BodyPane.hex);
  });
}
