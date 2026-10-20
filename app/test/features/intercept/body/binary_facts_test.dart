// Die zwei Sätze über einem binären Rumpf (HUM-058, Zeile 10): die
// Vermutung aus den ersten Bytes und der Hinweis, dass der Daemon nur die
// druckbaren Läufe durchsucht hat.
//
// Gegen welche Umkehrung jeder Test gerichtet ist:
//
// - Der Zweig `BodyKind.binary` in `bodyNotes` weg: `binary_body_hex_view`
//   wird rot.
// - `scannedAsStrings` sagt für jeden Typ ja (oder nimmt das Schema aus dem
//   Spiel): rot in `the strings line mirrors the detector`, weil ein JSON-Typ
//   im Daemon ganz durchsucht wird.
// - Ein Eintrag der Tabelle in `binary_facts.dart` falsch: rot in
//   `magic bytes`.

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/body/binary_facts.dart';
import 'package:humanitl/core/body/body_kind.dart';
import 'package:humanitl/core/body/body_parser.dart';
import 'package:humanitl/core/body/body_view.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/l10n/l10n.dart';

final AppLocalizations english = lookupAppLocalizations(const Locale('en'));
final AppLocalizations german = lookupAppLocalizations(const Locale('de'));

/// Ein PNG-Kopf und Rauschen.
final Uint8List png = Uint8List.fromList(<int>[
  0x89,
  0x50,
  0x4E,
  0x47,
  0x0D,
  0x0A,
  0x1A,
  0x0A,
  ...List<int>.generate(152, (int i) => (i * 37 + 11) % 256),
]);

void main() {
  group('binary_body_hex_view', () {
    test('a binary body names its guess and the strings scan', () {
      final ParsedBody parsed = parseBody(
        png,
        BodyKind.binary,
        const <Finding>[],
      );
      final List<String> notes = bodyNotes(
        parsed,
        BodyPane.hex,
        png.length,
        english,
        contentType: 'application/octet-stream',
      );
      expect(
        notes,
        containsAll(<String>[
          english.interceptBodyMimeGuess('image/png'),
          english.interceptBodyScannedAsStrings,
        ]),
      );
      expect(
        english.interceptBodyMimeGuess('image/png'),
        startsWith('The first bytes look like image/png.'),
      );
      expect(
        german.interceptBodyScannedAsStrings,
        contains('nur die druckbaren Zeichenketten'),
      );
    });

    test('a text body gets neither sentence', () {
      final Uint8List text = Uint8List.fromList(utf8.encode('{"a":1}'));
      final ParsedBody parsed = parseBody(
        text,
        BodyKind.json,
        const <Finding>[],
      );
      final List<String> notes = bodyNotes(
        parsed,
        BodyPane.hex,
        text.length,
        english,
        contentType: 'application/json',
      );
      expect(notes, isNot(contains(english.interceptBodyScannedAsStrings)));
      expect(notes.where((String note) => note.contains('look like')), isEmpty);
    });
  });

  test('magic bytes', () {
    Uint8List b(List<int> bytes) => Uint8List.fromList(bytes);
    expect(guessMimeFromMagic(png), 'image/png');
    expect(guessMimeFromMagic(b(<int>[0xFF, 0xD8, 0xFF, 0xE0])), 'image/jpeg');
    expect(guessMimeFromMagic(b(utf8.encode('GIF89a'))), 'image/gif');
    expect(guessMimeFromMagic(b(utf8.encode('%PDF-1.7'))), 'application/pdf');
    expect(
      guessMimeFromMagic(b(<int>[0x50, 0x4B, 0x03, 0x04, 0])),
      'application/zip',
    );
    expect(guessMimeFromMagic(b(<int>[0x1F, 0x8B, 8])), 'application/gzip');
    expect(
      guessMimeFromMagic(
        b(<int>[...utf8.encode('RIFF'), 1, 2, 3, 4, ...utf8.encode('WEBP')]),
      ),
      'image/webp',
    );
    expect(
      guessMimeFromMagic(
        b(<int>[...utf8.encode('RIFF'), 1, 2, 3, 4, ...utf8.encode('WAVE')]),
      ),
      isNull,
    );
    final List<int> tar = List<int>.filled(300, 0);
    tar.setRange(257, 262, utf8.encode('ustar'));
    expect(guessMimeFromMagic(b(tar)), 'application/x-tar');
    expect(guessMimeFromMagic(b(<int>[1, 2, 3, 4])), isNull);
    expect(guessMimeFromMagic(b(<int>[0x89, 0x50])), isNull);
  });

  test('the strings line mirrors the detector', () {
    final Uint8List noise = Uint8List.fromList(<int>[0xFF, 0xFE, 0x00, 0x81]);
    final Uint8List utf = Uint8List.fromList(utf8.encode('Müller'));
    // Mit Typ entscheidet allein der Typ (`ContentType::is_textual`).
    expect(scannedAsStrings('application/octet-stream', utf), isTrue);
    expect(scannedAsStrings('image/png', noise), isTrue);
    expect(scannedAsStrings('application/json', noise), isFalse);
    expect(scannedAsStrings('Application/JSON; charset=utf-8', noise), isFalse);
    expect(scannedAsStrings('text/plain', noise), isFalse);
    expect(scannedAsStrings('application/vnd.api+json', noise), isFalse);
    expect(scannedAsStrings('application/atom+xml', noise), isFalse);
    expect(scannedAsStrings('application/graphql', noise), isFalse);
    // Ohne Typ entscheidet, ob die Bytes gültiges UTF-8 sind.
    expect(scannedAsStrings('', noise), isTrue);
    expect(scannedAsStrings(null, utf), isFalse);
  });
}
