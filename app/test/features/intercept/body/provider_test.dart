// Der Weg vom Verweis zur Ansicht: holen, auspacken, merken -- und die Frage,
// ob diese Bytes überhaupt die sind, auf denen der Daemon gesucht hat.
// Geprüft an den Strömen, die diesen Weg täuschen sollen: eine Bombe, ein
// abgeschnittenes gzip, ein zweigliedriges gzip, eine Kodierung, die es hier
// nicht gibt, und Magic Bytes ohne Kopfzeile.

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/intercept/body/body_decode.dart';
import 'package:humanitl/features/intercept/body/body_kind.dart';
import 'package:humanitl/features/intercept/body/body_parser.dart';
import 'package:humanitl/features/intercept/body/body_view.dart';
import 'package:humanitl/features/intercept/providers/flow_body_provider.dart';

import 'harness.dart';

BodyRef refFor(int size, {String contentType = '', bool truncated = false}) =>
    BodyRef(
      sha256: List<int>.filled(32, size % 251),
      size: size,
      contentType: contentType,
      truncated: truncated,
    );

Uint8List packed(String text) =>
    Uint8List.fromList(gzip.encode(utf8.encode(text)));

Uint8List cut(Uint8List bytes, int off) =>
    Uint8List.sublistView(bytes, 0, bytes.length - off);

void main() {
  group('what the header says decides', () {
    test('a gzip body is unpacked when the header says so', () {
      final Uint8List body = packed('{"a":1}');
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: body),
        refFor(body.length, contentType: 'application/json'),
        'gzip',
      );
      expect(load.decompressed, isTrue);
      expect(load.aligned, isTrue);
      expect(utf8.decode(load.bytes), '{"a":1}');
      expect(load.kind, BodyKind.json);
      expect(load.problem, isNull);
    });

    test('magic bytes without a header are left alone', () {
      // Der Daemon durchsucht die Rohbytes, wenn die Anfrage nichts ankündigt.
      // Wer hier trotzdem auspackt, zeichnet Fundstellen aus einem Byteraum,
      // den der Daemon nie gesehen hat.
      final Uint8List body = packed('{"a":1}');
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: body),
        refFor(body.length),
        '',
      );
      expect(load.decompressed, isFalse);
      expect(load.aligned, isTrue);
      expect(load.bytes, body);
    });

    test('an encoding this view cannot unpack places no finding', () {
      final Uint8List body = Uint8List.fromList(<int>[1, 2, 3, 4, 5, 6]);
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: body),
        refFor(body.length, contentType: 'application/json'),
        'br',
      );
      expect(load.aligned, isFalse);
      expect(load.problem, BodyProblem.undecodedEncoding);
      expect(load.bytes, body);
      final ParsedBody parsed = parseLoadedBody(load, <Finding>[
        bodyFinding(start: 0, end: 3),
      ]);
      expect(parsed.findingsPlaced, isFalse);
      expect(parsed.placedFindings, isEmpty);
      for (final BodyPane pane in BodyPane.values) {
        expect(unmarkedFindings(parsed, pane, load.bytes.length), <int>{0});
      }
      expect(
        bodyNotes(parsed, BodyPane.hex, load.bytes.length, english),
        contains(english.interceptBodyFindingsNotPlaced(1)),
      );
    });

    test('a chain of encodings counts as unsupported', () {
      expect(encodingOf('gzip, br'), BodyEncoding.unsupported);
      expect(encodingOf('GZIP'), BodyEncoding.unsupported);
      expect(encodingOf('gzip'), BodyEncoding.gzip);
      expect(encodingOf('x-gzip'), BodyEncoding.gzip);
      expect(encodingOf('deflate'), BodyEncoding.zlib);
      expect(encodingOf(''), BodyEncoding.identity);
      expect(encodingOf('identity'), BodyEncoding.identity);
    });

    test('the header is read case-insensitively from the request', () {
      expect(
        contentEncodingOf(<Header>[
          Header(name: 'Content-Type', value: 'application/json'.codeUnits),
          Header(name: 'Content-Encoding', value: ' GZIP '.codeUnits),
        ]),
        'gzip',
      );
      expect(contentEncodingOf(const <Header>[]), '');
    });
  });

  group('a stream that does not end where it says', () {
    test('a halved gzip is named, not passed off as complete', () {
      // Der Dekodierer von Dart wirft dabei nicht: er liefert die Teilausgabe
      // und meldet Erfolg. Erst der Abschluss verrät den Abbruch.
      final Uint8List whole = packed('x' * 4096);
      final Uint8List half = Uint8List.sublistView(whole, 0, whole.length ~/ 2);
      final InflatedBody inflated = inflateBody(half, BodyEncoding.gzip);
      expect(inflated.aborted, isTrue);
      expect(inflated.aligned, isFalse);
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: half),
        refFor(half.length, contentType: 'text/plain'),
        'gzip',
      );
      expect(
        load.problem,
        anyOf(BodyProblem.truncatedStream, BodyProblem.undecodedEncoding),
      );
      expect(load.aligned, isFalse);
    });

    test('a gzip without its trailer is named', () {
      final Uint8List whole = packed('x' * 4096);
      final InflatedBody inflated = inflateBody(
        cut(whole, 8),
        BodyEncoding.gzip,
      );
      expect(inflated.aligned, isFalse);
      expect(inflated.aborted, isTrue);
    });

    test('a zlib without its adler is named', () {
      final Uint8List whole = Uint8List.fromList(
        zlib.encode(utf8.encode('y' * 4096)),
      );
      expect(inflateBody(whole, BodyEncoding.zlib).aligned, isTrue);
      expect(inflateBody(cut(whole, 4), BodyEncoding.zlib).aligned, isFalse);
    });

    test('a two member gzip is not treated as the same bytes', () {
      // Dart entpackt beide Glieder, `flate2` im Daemon heute nur das erste.
      // Solange das so ist, sind die Byteräume verschieden, und diese Ansicht
      // markiert lieber nichts als etwas Falsches.
      final Uint8List two = Uint8List.fromList(<int>[
        ...packed('first member '),
        ...packed('second member'),
      ]);
      final InflatedBody inflated = inflateBody(two, BodyEncoding.gzip);
      expect(inflated.aligned, isFalse);
    });

    test('a complete stream verifies', () {
      expect(inflateBody(packed('hello'), BodyEncoding.gzip).aligned, isTrue);
      expect(
        utf8.decode(inflateBody(packed('hello'), BodyEncoding.gzip).bytes),
        'hello',
      );
    });

    test('adler32 matches the value zlib writes', () {
      final Uint8List body = Uint8List.fromList(utf8.encode('Wikipedia'));
      // Bekannter Wert der Referenzimplementierung.
      expect(adler32(body), 0x11E60398);
    });
  });

  group('a bomb and a broken stream', () {
    test('a gzip bomb stops at the cap', () {
      // Vierundsechzig Mebibyte Nullen: achtmal die Grenze, damit die
      // Zusicherung ohne Abbruch nicht zufällig trotzdem hielte.
      final Uint8List bomb = Uint8List.fromList(
        gzip.encode(Uint8List(64 * 1024 * 1024)),
      );
      expect(bomb.length, lessThan(bodyMaxBytes));
      final InflatedBody inflated = inflateBody(bomb, BodyEncoding.gzip);
      expect(inflated.overflowed, isTrue);
      expect(inflated.aborted, isTrue);
      expect(inflated.aligned, isFalse);
      expect(inflated.bytes.length, lessThan(64 * 1024 * 1024));
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: bomb),
        refFor(bomb.length),
        'gzip',
      );
      expect(load.kind, BodyKind.tooLarge);
      expect(load.problem, BodyProblem.tooLarge);
    });

    test('a broken gzip is not reported as a lying content type', () {
      // Vorher las der Mensch hier "content type says text, bytes are not
      // text" -- eine falsche Erklärung für ein echtes Problem.
      final Uint8List whole = packed('x' * 4096);
      final Uint8List broken = Uint8List.fromList(whole)
        ..[whole.length - 5] ^= 0xFF;
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: broken),
        refFor(broken.length, contentType: 'text/plain'),
        'gzip',
      );
      expect(load.disputedType, isFalse);
      expect(load.aligned, isFalse);
      expect(
        load.problem,
        anyOf(BodyProblem.truncatedStream, BodyProblem.undecodedEncoding),
      );
    });

    test('garbage that only looks like zlib keeps its bytes', () {
      final Uint8List bytes = Uint8List.fromList(<int>[0x78, 0x9C, 1, 2, 3, 4]);
      final InflatedBody inflated = inflateBody(bytes, BodyEncoding.zlib);
      expect(inflated.bytes, bytes);
      expect(inflated.decompressed, isFalse);
      expect(inflated.aligned, isFalse);
    });
  });

  group('the transport', () {
    test('a short stream is incomplete, never empty', () {
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: bytesOf('0123456789'), short: true),
        refFor(100),
        '',
      );
      expect(load.problem, BodyProblem.incomplete);
      expect(load.kind, isNot(BodyKind.empty));
      expect(load.bytes, isNotEmpty);
    });

    test('a recorded prefix is not called incomplete', () {
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: bytesOf('0123456789')),
        refFor(100, truncated: true),
        '',
      );
      expect(load.problem, isNull);
    });

    test('a body over the cap is too large and keeps what arrived', () {
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: bytesOf('{"a":1}'), overflowed: true),
        refFor(bodyMaxBytes + 1),
        '',
      );
      expect(load.kind, BodyKind.tooLarge);
      expect(load.problem, BodyProblem.tooLarge);
      expect(load.bytes, isNotEmpty);
    });
  });

  group('the cache', () {
    RawBody entry([int size = 0]) => RawBody(bytes: Uint8List(size));

    test('the key separates what makes a different display', () {
      final BodyRef base = refFor(10, contentType: 'application/json');
      expect(cacheKeyOf(base), cacheKeyOf(base));
      expect(
        cacheKeyOf(base),
        isNot(cacheKeyOf(base.copyWith(contentType: 'text/plain'))),
      );
      expect(
        cacheKeyOf(base),
        isNot(cacheKeyOf(base.copyWith(truncated: true))),
      );
    });

    test('it keeps the newest and drops by count', () {
      final BodyCache cache = BodyCache();
      for (int i = 0; i < bodyCacheEntries + 5; i++) {
        cache.write('key$i', entry());
      }
      expect(cache.length, bodyCacheEntries);
      expect(cache.read('key0'), isNull);
      expect(cache.read('key${bodyCacheEntries + 4}'), isNotNull);
    });

    test('it drops by bytes as well', () {
      final BodyCache cache = BodyCache();
      for (int i = 0; i < 8; i++) {
        cache.write('key$i', entry(bodyCacheBytes ~/ 4));
      }
      expect(cache.bytes, lessThanOrEqualTo(bodyCacheBytes));
      expect(cache.length, lessThan(8));
    });

    test('a read makes an entry the youngest', () {
      final BodyCache cache = BodyCache();
      for (int i = 0; i < bodyCacheEntries; i++) {
        cache.write('key$i', entry());
      }
      expect(cache.read('key0'), isNotNull);
      cache.write('fresh', entry());
      expect(cache.read('key0'), isNotNull);
      expect(cache.read('key1'), isNull);
    });
  });
}
