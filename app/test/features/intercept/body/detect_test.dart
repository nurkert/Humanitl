// Was in einem Rumpf steht, entscheidet, was ein Mensch zu sehen bekommt.
// Diese Datei prüft die Erkennung an den Fällen, die eine Anzeige täuschen
// könnten, nicht nur am Normalfall.

import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/intercept/body/body_kind.dart';

import 'harness.dart';

void main() {
  test('detect_empty', () {
    expect(detectBodyKind(Uint8List(0), 'application/json'), BodyKind.empty);
  });

  test('detect_json_without_ct', () {
    expect(detectBodyKind(bytesOf('  {"a": 1}'), null), BodyKind.json);
    expect(detectBodyKind(bytesOf('[1, 2, 3]'), ''), BodyKind.json);
  });

  test('detect_json_by_suffix_content_type', () {
    expect(
      detectBodyKind(bytesOf('nothing structural'), 'application/vnd.api+json'),
      BodyKind.json,
    );
  });

  test('detect_content_type_parameters_are_ignored', () {
    expect(
      detectBodyKind(bytesOf('{}'), 'Application/JSON; charset=utf-8'),
      BodyKind.json,
    );
  });

  test('detect_form_by_pattern', () {
    expect(detectBodyKind(bytesOf('a=1&b=2&c='), null), BodyKind.form);
    expect(detectBodyKind(bytesOf('a=1 & b=2'), null), BodyKind.text);
  });

  test('detect_form_pattern_survives_the_sniff_window', () {
    final String long = List<String>.generate(
      400,
      (int i) => 'key$i=value$i',
    ).join('&');
    expect(long.length, greaterThan(bodySniffWindow));
    expect(detectBodyKind(bytesOf(long), null), BodyKind.form);
  });

  test('detect_binary_ratio', () {
    final Uint8List noise = Uint8List.fromList(
      List<int>.generate(600, (int i) => i.isEven ? 0x41 : 0x80 + (i % 60)),
    );
    expect(detectBodyKind(noise, null), BodyKind.binary);
  });

  test('detect_utf8_text_is_not_binary', () {
    // Umlaute und ein Emoji sind Mehrbyte-Folgen; wer nur auf ASCII prüft,
    // erklärt einen deutschen Satz zu Binärdaten.
    expect(
      detectBodyKind(bytesOf('Größe: 12 €, Stimmung: 😀' * 20), null),
      BodyKind.text,
    );
  });

  test('detect_bom_before_json', () {
    final Uint8List bytes = Uint8List.fromList(<int>[
      0xEF,
      0xBB,
      0xBF,
      ...bytesOf('{"a":1}'),
    ]);
    expect(detectBodyKind(bytes, null), BodyKind.json);
  });

  test('detect_xml_and_html', () {
    expect(
      detectBodyKind(bytesOf('<?xml version="1.0"?><a/>'), null),
      BodyKind.xml,
    );
    expect(detectBodyKind(bytesOf('<html><body>hi'), null), BodyKind.xml);
    expect(detectBodyKind(bytesOf('plain'), 'text/html'), BodyKind.xml);
  });

  test('detect_text_fallback', () {
    expect(detectBodyKind(bytesOf('hello there'), null), BodyKind.text);
    expect(detectBodyKind(bytesOf('hello'), 'text/csv'), BodyKind.text);
  });

  test('detect_too_large', () {
    expect(
      detectBodyKind(bytesOf('{}'), null, totalSize: bodyMaxBytes + 1),
      BodyKind.tooLarge,
    );
  });

  test('detect_exactly_eight_mebibytes_is_not_too_large', () {
    // Die Grenze ist strikt größer. Genau 8 MiB wird noch gezeigt; wer hier
    // `>=` schreibt, nimmt dem Menschen einen Rumpf weg, der erlaubt ist.
    expect(
      detectBodyKind(bytesOf('{}'), null, totalSize: bodyMaxBytes),
      BodyKind.json,
    );
  });

  test('detect_lying_content_type_shows_the_bytes', () {
    // Ein PNG, das sich als JSON ausgibt. Die Kopfzeile ist eine Behauptung
    // des Absenders, die Bytes sind der Beleg, und der Mensch entscheidet über
    // den Beleg.
    final Uint8List png = Uint8List.fromList(<int>[
      0x89,
      0x50,
      0x4E,
      0x47,
      0x0D,
      0x0A,
      0x1A,
      0x0A,
      ...List<int>.generate(200, (int i) => (i * 7) % 256),
    ]);
    expect(detectBodyKind(png, 'application/json'), BodyKind.binary);
    expect(bodyTypeIsDisputed(png, 'application/json'), isTrue);
  });

  test('detect_binary_marker_does_not_beat_the_ratio', () {
    // Eine Datei, die mit `{` beginnt und dahinter Binärdaten trägt, ist keine
    // JSON, die gleich scheitert, sondern eine Datei mit gewählter erster
    // Zeile.
    final Uint8List bytes = Uint8List.fromList(<int>[
      0x7B,
      ...List<int>.generate(300, (int i) => 0x80 + (i % 60)),
    ]);
    expect(detectBodyKind(bytes, null), BodyKind.binary);
  });

  test('detect_octet_stream_json_stays_readable', () {
    // Umgekehrt gilt die Regel nicht: eine Kopfzeile, die weniger behauptet,
    // als dasteht, ist keine Täuschung.
    expect(
      detectBodyKind(bytesOf('{"a":1}'), 'application/octet-stream'),
      BodyKind.json,
    );
    expect(
      bodyTypeIsDisputed(bytesOf('{"a":1}'), 'application/octet-stream'),
      isFalse,
    );
  });

  test('every_kind_offers_a_view_that_shows_something', () {
    for (final BodyKind kind in BodyKind.values) {
      if (kind == BodyKind.empty) {
        expect(bodyViewsFor(kind), isEmpty, reason: '$kind');
        continue;
      }
      expect(bodyViewsFor(kind), isNotEmpty, reason: '$kind');
    }
  });
}
