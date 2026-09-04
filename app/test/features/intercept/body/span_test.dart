// Versätze und Bereinigung. Beide hängen an derselben Zahl, und wenn eine der
// beiden sie verschiebt, sitzt jede Fundmarkierung daneben.

import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/intercept/body/body_span.dart';

import 'harness.dart';

void main() {
  test('byte_to_char_offset_umlauts_and_emoji', () {
    // `a` 1 Byte, `ä` 2, `€` 3, das Emoji 4 Bytes und zwei Code-Units.
    final Uint8List bytes = bytesOf('aä€\u{1F600}b');
    expect(bytes.length, 11);
    expect(byteToCharOffset(bytes, 0), 0);
    expect(byteToCharOffset(bytes, 1), 1);
    expect(byteToCharOffset(bytes, 3), 2);
    expect(byteToCharOffset(bytes, 6), 3);
    expect(byteToCharOffset(bytes, 10), 5);
    expect(byteToCharOffset(bytes, 11), 6);
  });

  test('byte_to_char_offset_survives_broken_utf8', () {
    // Der Dekodierer der Anzeige ersetzt kaputte Folgen; diese Rechnung
    // benutzt denselben, also stimmen beide Zahlen überein.
    final Uint8List bytes = Uint8List.fromList(<int>[0x41, 0xC3, 0x28, 0x42]);
    expect(byteToCharOffset(bytes, 4), 4);
    expect(byteToCharOffset(bytes, 1), 1);
  });

  test('sanitize_keeps_the_length', () {
    const String hostile = 'a\u{202E}b\u{200B}c d e';
    expect(sanitizeBodyText(hostile).length, hostile.length);
  });

  test('sanitize_removes_the_right_to_left_override', () {
    // Ohne diese Ersetzung zeigt ein Textfeld `123654`, während die Bytes
    // `123` `U+202E` `456` gesendet werden -- der Mensch entscheidet dann über
    // etwas anderes, als er liest.
    const String reversed = '123\u{202E}456';
    final String safe = sanitizeBodyText(reversed);
    expect(safe.contains('\u{202E}'), isFalse);
    expect(safe, '123${bodyReplacementChar}456');
  });

  test('sanitize_keeps_tab_newline_and_return', () {
    expect(sanitizeBodyText('a\tb\nc\rd'), 'a\tb\nc\rd');
  });

  test('sanitize_replaces_every_invisible_of_the_list', () {
    // Die Liste steht hier ausgeschrieben und nicht im Code: sie ist die
    // zweite Meinung. Wer einen Codepunkt aus der Umsetzung entfernt, wird
    // hier rot, und das ist der Sinn der Sache.
    const List<int> hidden = <int>[
      // C0 und Delete.
      0x00, 0x01, 0x07, 0x08, 0x0B, 0x0C, 0x1B, 0x1F, 0x7F,
      // C1.
      0x80, 0x85, 0x9F,
      // Formatzeichen der Basisebene.
      0x00AD, 0x0600, 0x0605, 0x061C, 0x06DD, 0x070F, 0x08E2, 0x180E,
      0x200B, 0x200C, 0x200D, 0x200E, 0x200F,
      0x2028, 0x2029,
      0x202A, 0x202B, 0x202C, 0x202D, 0x202E,
      0x2060, 0x2064, 0x2066, 0x2067, 0x2068, 0x2069, 0x206F,
      // Symmetric Swapping: kippt die Spiegelung von Klammern.
      0x206A, 0x206B, 0x206C, 0x206D, 0x206E,
      0xFEFF, 0xFFF9, 0xFFFA, 0xFFFB,
      // Formatzeichen außerhalb der Basisebene, als Ersatzpaar kodiert.
      0x110BD, 0x110CD, 0x13430, 0x1BCA0, 0x1D173,
      0xE0001, 0xE0020, 0xE0041, 0xE007F, 0xE0100, 0xE01EF,
    ];
    for (final int rune in hidden) {
      final String text = 'a${String.fromCharCode(rune)}b';
      final String safe = sanitizeBodyText(text);
      expect(
        safe.length,
        text.length,
        reason: 'length ${rune.toRadixString(16)}',
      );
      expect(
        safe.contains(String.fromCharCode(rune)),
        isFalse,
        reason: rune.toRadixString(16),
      );
      expect(safe.startsWith('a'), isTrue, reason: rune.toRadixString(16));
      expect(safe.endsWith('b'), isTrue, reason: rune.toRadixString(16));
    }
  });

  test('sanitize_keeps_what_a_person_reads', () {
    // Die Gegenprobe: was gezeigt werden darf, bleibt unverändert. Ohne sie
    // bestünde die Prüfung darüber auch, wenn alles ersetzt würde.
    for (final String text in <String>[
      'plain ascii 0123',
      'Größe: 12 €',
      'Stimmung: \u{1F600}',
      'kanji: \u{6F22}\u{5B57}',
      'a\tb\nc\rd',
      'accents: e\u{0301}',
    ]) {
      expect(sanitizeBodyText(text), text, reason: text);
    }
  });

  test('many findings in one pass keep their offsets across the slices', () {
    // Die Versätze werden gestückelt gerechnet, und jede angefragte Stelle ist
    // eine Stückgrenze. Fällt eine Mehrbyte-Folge auf so eine Grenze, darf
    // nichts verrutschen -- sonst säße der Unterstrich auf dem Nachbarzeichen.
    const String source =
        'a\u{00E4}b\u{20AC}c\u{1F600}d\u{6F22}e mail=zz@example.org '
        'key=ghp_A1B2C3D4 \u{00FC}ber';
    final Uint8List bytes = bytesOf(source);
    final List<String> needles = <String>[
      '\u{00E4}',
      '\u{20AC}',
      '\u{1F600}',
      '\u{6F22}',
      'zz@example.org',
      'ghp_A1B2C3D4',
      '\u{00FC}ber',
    ];
    final List<Finding> raw = <Finding>[
      for (final String needle in needles)
        bodyFinding(
          start: bytesOf(source.substring(0, source.indexOf(needle))).length,
          end:
              bytesOf(source.substring(0, source.indexOf(needle))).length +
              bytesOf(needle).length,
        ),
    ];
    final List<BodyFinding> mapped = mapBodyFindings(bytes, raw);
    expect(mapped, hasLength(needles.length));
    for (int i = 0; i < needles.length; i++) {
      expect(
        source.substring(mapped[i].charStart, mapped[i].charEnd),
        needles[i],
        reason: needles[i],
      );
      expect(mapped[i].needle, needles[i], reason: needles[i]);
    }
  });

  test('an offset inside a multibyte sequence does not run past it', () {
    // Eine Stelle mitten in einer Folge gehört vor das Zeichen, nicht dahinter.
    final Uint8List bytes = bytesOf('a\u{1F600}b');
    expect(byteToCharOffset(bytes, 2), 1);
    expect(byteToCharOffset(bytes, 5), 3);
  });

  test('a finding behind the loaded bytes keeps its name, not a place', () {
    final List<BodyFinding> mapped = mapBodyFindings(
      bytesOf('short'),
      <Finding>[bodyFinding(start: 900000, end: 900006)],
    );
    expect(mapped, hasLength(1));
    expect(mapped.single.hasRange, isFalse);
    expect(mapped.single.needle, isEmpty);
    expect(mapped.single.byteStart, 900000);
  });

  test('map_findings_lands_on_the_match_after_umlauts', () {
    const String source = '{"kunde":"Müller","mail":"a@b.de"}';
    final Uint8List bytes = bytesOf(source);
    // Der Daemon zählt Bytes; `ü` ist zwei davon, also liegt der Versatz um
    // eins hinter dem Zeichenindex.
    final int start = bytesOf(source.substring(0, source.indexOf('a@b.de')))
        .length;
    final List<BodyFinding> mapped = mapBodyFindings(bytes, <Finding>[
      bodyFinding(start: start, end: start + 6),
    ]);
    expect(mapped, hasLength(1));
    expect(mapped.single.needle, 'a@b.de');
    expect(
      source.substring(mapped.single.charStart, mapped.single.charEnd),
      'a@b.de',
    );
  });

  test('findings_outside_the_body_are_not_mapped', () {
    final List<BodyFinding> mapped = mapBodyFindings(bytesOf('abc'), <Finding>[
      const Finding(
        kind: 'api_key:github',
        location: FindingLocation.header,
        headerName: 'authorization',
        spanStart: 0,
        spanEnd: 3,
        tier: FindingTier.checksum,
      ),
    ]);
    expect(mapped, isEmpty);
  });

  test('an_unknown_kind_is_treated_as_a_secret', () {
    // Lieber zu ernst als zu leicht: ein Fund, den diese Tabelle nicht kennt,
    // bekommt den Ton des Geheimnisses.
    expect(bodyFindingTone('custom:acme'), BodyFindingTone.secret);
    expect(bodyFindingTone('email'), BodyFindingTone.personal);
    expect(bodyFindingTone('api_key:github'), BodyFindingTone.secret);
  });
}
