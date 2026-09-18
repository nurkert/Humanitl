/// Die drei Markierungen des Editors, als Farbe und als Stil (HUM-047).
///
/// Der Diff-Glow ist ein Signature-Element (BACKLOG.md 5), und was ein
/// Signature-Element ausmacht, sind genau die Werte, die hier stehen: der
/// Akzent, ein Pixel Unterstrich, zehn Prozent Fläche. Ein Test, der nur zählt,
/// wie viele Markierungen es gibt, ließe jede Farbe durch.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/editor/model/draft.dart';
import 'package:humanitl/features/editor/providers/draft_provider.dart';

void main() {
  group('HEditorDecorations', () {
    test('each kind has its own colour, and the glow is the accent', () {
      final HTokens tokens = HTokens.dark;

      expect(
        HEditorDecorations.colorOf(tokens, HEditorDecorationKind.secret),
        tokens.state.error,
      );
      expect(
        HEditorDecorations.colorOf(tokens, HEditorDecorationKind.pii),
        tokens.state.held,
      );
      expect(
        HEditorDecorations.colorOf(tokens, HEditorDecorationKind.replaced),
        tokens.colors.accent,
      );
      // Drei verschiedene Farben, nicht dreimal dieselbe: Der Fund, das
      // Geheimnis und die erledigte Stelle sagen drei verschiedene Dinge.
      expect(<Color>{
        for (final HEditorDecorationKind kind in HEditorDecorationKind.values)
          HEditorDecorations.colorOf(tokens, kind),
      }, hasLength(3));
    });

    test('the glow carries the accent area, a finding does not', () {
      final HTokens tokens = HTokens.dark;
      const TextStyle base = TextStyle();

      final TextStyle glow = HEditorDecorations.styleOf(
        tokens,
        HEditorDecorationKind.replaced,
        base,
      );
      final TextStyle finding = HEditorDecorations.styleOf(
        tokens,
        HEditorDecorationKind.pii,
        base,
      );

      expect(
        glow.backgroundColor,
        tokens.colors.accent.withValues(alpha: HEditorDecorations.glowAlpha),
      );
      expect(HEditorDecorations.glowAlpha, 0.10);
      expect(glow.decoration, TextDecoration.underline);
      expect(glow.decorationStyle, TextDecorationStyle.solid);
      expect(glow.decorationThickness, 1);
      // Der Fund bekommt keine Fläche, sondern eine Welle: Die beiden
      // Aussagen müssen sich auch ohne Farbe unterscheiden lassen.
      expect(finding.backgroundColor, isNull);
      expect(finding.decorationStyle, TextDecorationStyle.wavy);
    });

    test('slice cuts the text where the marks change', () {
      const String text = 'abcdefgh';

      final List<({int start, int end, HEditorDecorationKind? kind})> parts =
          HEditorDecorations.slice(text, <HEditorDecoration>[
            const HEditorDecoration(
              start: 2,
              end: 4,
              kind: HEditorDecorationKind.pii,
            ),
            const HEditorDecoration(
              start: 6,
              end: 8,
              kind: HEditorDecorationKind.replaced,
            ),
          ]);

      expect(parts, hasLength(4));
      expect(parts[0], (start: 0, end: 2, kind: null));
      expect(parts[1], (start: 2, end: 4, kind: HEditorDecorationKind.pii));
      expect(parts[2], (start: 4, end: 6, kind: null));
      expect(parts[3], (
        start: 6,
        end: 8,
        kind: HEditorDecorationKind.replaced,
      ));
      // Die Stücke decken den Text lückenlos ab; sonst fehlten Zeichen in der
      // Anzeige.
      expect(
        <String>[
          for (final ({int start, int end, HEditorDecorationKind? kind}) part
              in parts)
            text.substring(part.start, part.end),
        ].join(),
        text,
      );
    });

    test('the later mark wins, so a replaced span stops warning', () {
      final List<({int start, int end, HEditorDecorationKind? kind})> parts =
          HEditorDecorations.slice('abcd', <HEditorDecoration>[
            const HEditorDecoration(
              start: 0,
              end: 4,
              kind: HEditorDecorationKind.secret,
            ),
            const HEditorDecoration(
              start: 0,
              end: 4,
              kind: HEditorDecorationKind.replaced,
            ),
          ]);

      expect(parts.single.kind, HEditorDecorationKind.replaced);
    });

    test('a mark outside the text is clamped, never thrown', () {
      final List<({int start, int end, HEditorDecorationKind? kind})> parts =
          HEditorDecorations.slice('ab', <HEditorDecoration>[
            const HEditorDecoration(
              start: 1,
              end: 99,
              kind: HEditorDecorationKind.pii,
            ),
          ]);

      expect(parts, hasLength(2));
      expect(parts.last, (start: 1, end: 2, kind: HEditorDecorationKind.pii));
    });
  });

  group('charSpanOfBytes', () {
    test('a byte offset behind an umlaut is not a code-unit offset', () {
      // „Grüsse a@x.de": das `ü` ist zwei Bytes, also liegt die Adresse im
      // Byte-Raum eins weiter rechts als im Zeichen-Raum.
      const String value = 'Grüsse a@x.de';
      expect(value.indexOf('a@x.de'), 7);

      final ({int start, int end}) span = charSpanOfBytes(value, 8, 14);

      expect(span.start, 7);
      expect(span.end, 13);
      expect(value.substring(span.start, span.end), 'a@x.de');
    });

    test('plain ascii maps one to one', () {
      expect(charSpanOfBytes('mail a@x.de', 5, 11), (start: 5, end: 11));
    });

    test('a span behind the end has no place at all', () {
      expect(charSpanOfBytes('short', 99, 120), (start: 0, end: 0));
    });
  });

  group('DraftLocation', () {
    test('a header location is lowercase, whatever the daemon wrote', () {
      const Finding finding = Finding(
        kind: 'email',
        location: FindingLocation.header,
        headerName: 'X-User',
        spanStart: 0,
        spanEnd: 6,
        tier: FindingTier.regex,
      );

      expect(DraftLocation.of(finding).headerName, 'x-user');
      expect(DraftLocation.header('X-User'), DraftLocation.of(finding));
    });
  });
}
