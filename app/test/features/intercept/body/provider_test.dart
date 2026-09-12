// Der Weg vom Verweis zur Ansicht: fragen, einordnen, merken -- und die Frage,
// ob diese Bytes überhaupt die sind, auf denen der Daemon gesucht hat.
//
// Seit HUM-119 packt die App nicht mehr selbst aus. Sie fragt `GetBody` mit
// `decoded = true` und liest `BodyChunk.encoding_left`: leer heißt „das ist der
// Byteraum der Funde", ein Name heißt „hier liegt noch etwas darauf". Geprüft
// wird deshalb nicht mehr ein Entpacker, sondern dass genau diese Auskunft
// gefragt, weitergereicht und geglaubt wird. Der Entpacker selbst steht im
// Daemon und wird dort geprüft (`daemon/crates/ipc/tests/get_body.rs`).

import 'dart:convert';
import 'dart:io' show gzip;
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/body/body_decode.dart';
import 'package:humanitl/core/body/body_kind.dart';
import 'package:humanitl/core/body/body_parser.dart';
import 'package:humanitl/core/body/body_providers.dart';
import 'package:humanitl/core/body/body_view.dart';
import 'package:humanitl/core/body/flow_body_provider.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

import 'harness.dart';

BodyRef refFor(
  int size, {
  String contentType = '',
  bool truncated = false,
  String contentEncoding = '',
}) => BodyRef(
  sha256: List<int>.filled(32, size % 251),
  size: size,
  contentType: contentType,
  truncated: truncated,
  contentEncoding: contentEncoding,
);

/// Ein Fake, dessen `GetBody` mitten im Strom aufhört.
///
/// Kein Stück trägt `last`; der Daemon täte das nur, wenn die Verbindung
/// abbricht. Genau dann darf die Ansicht den Rumpf nicht für vollständig
/// halten.
class _EndlessClient extends FakeDaemonClient {
  @override
  Stream<BodyChunk> getBodyChunks(BodyRef ref) => Stream<BodyChunk>.value(
    BodyChunk(
      data: bytesOf('{"half": "arrived and then nothing"}'),
      last: false,
    ),
  );
}

/// Der Schlüssel, unter dem der Fake die Bytes zu [reference] hält.
String keyOf(BodyRef reference) => reference.sha256
    .map((int byte) => byte.toRadixString(16).padLeft(2, '0'))
    .join();

void main() {
  group('what the daemon says about the bytes decides', () {
    test('bytes the daemon unpacked carry the findings', () {
      // `encoding_left` ist leer: Der Daemon hat `br` abgenommen, und der
      // Bereich des Fundes zeigt in genau diese Bytes.
      const String plain = '{"email":"a@b.de"}';
      final int start = plain.indexOf('a@b.de');
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: bytesOf(plain)),
        refFor(85, contentType: 'application/json', contentEncoding: 'br'),
      );
      expect(load.aligned, isTrue);
      expect(load.decompressed, isTrue);
      expect(load.problem, isNull);
      expect(load.kind, BodyKind.json);
      final ParsedBody parsed = parseLoadedBody(load, <Finding>[
        bodyFinding(start: start, end: start + 6),
      ]);
      expect(parsed.findingsPlaced, isTrue);
      expect(parsed.placedFindings, hasLength(1));
    });

    test('unsupported_encoding_names_itself', () {
      // `zstd` bleibt auf den Bytes liegen. Dann werden die Rohbytes gezeigt,
      // der Fund behält seinen Namen und verliert seine Stelle, und der Satz
      // nennt die Kodierung.
      final Uint8List body = Uint8List.fromList(<int>[1, 2, 3, 4, 5, 6]);
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: body, encodingLeft: 'zstd'),
        refFor(6, contentType: 'application/json', contentEncoding: 'zstd'),
      );
      expect(load.aligned, isFalse);
      expect(load.decompressed, isFalse);
      expect(load.encoding, 'zstd');
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
      final List<String> notes = bodyNotes(
        parsed,
        BodyPane.hex,
        load.bytes.length,
        english,
      );
      expect(notes, contains(english.interceptBodyEncodingUndecoded('zstd')));
      expect(notes, contains(english.interceptBodyFindingsNotPlaced(1)));
    });

    test('a chain keeps its whole name', () {
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: bytesOf('packed twice'), encodingLeft: 'gzip, br'),
        refFor(12, contentEncoding: 'gzip, br'),
      );
      expect(load.encoding, 'gzip, br');
      expect(load.aligned, isFalse);
    });

    test('magic bytes without an encoding are left alone', () {
      // Der Daemon durchsucht die Rohbytes, wenn die Anfrage nichts
      // ankündigt. Wer hier trotzdem etwas vermutete, zeichnete Fundstellen
      // aus einem Byteraum, den der Daemon nie gesehen hat.
      final Uint8List body = Uint8List.fromList(<int>[
        0x1f,
        0x8b,
        0x08,
        ...List<int>.filled(32, 0),
      ]);
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: body),
        refFor(body.length),
      );
      expect(load.aligned, isTrue);
      expect(load.decompressed, isFalse);
      expect(load.bytes, body);
    });

    test('a body still packed is not called a lying content type', () {
      // Vorher las der Mensch hier "content type says text, bytes are not
      // text" -- eine falsche Erklärung für ein echtes Problem.
      final Uint8List body = Uint8List.fromList(<int>[
        0x28,
        0xb5,
        0x2f,
        0xfd,
        ...List<int>.generate(64, (int i) => (i * 13) % 256),
      ]);
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: body, encodingLeft: 'zstd'),
        refFor(body.length, contentType: 'text/plain', contentEncoding: 'zstd'),
      );
      expect(load.disputedType, isFalse);
      expect(load.problem, BodyProblem.undecodedEncoding);
    });
  });

  group('the request the provider sends', () {
    test('it always asks the daemon to unpack', () {
      final BodyRef reference = refFor(
        85,
        contentType: 'application/json',
        contentEncoding: 'br',
      );
      final BodyRef asked = reference.asking('br');
      expect(asked.decoded, isTrue);
      expect(asked.contentEncoding, 'br');
    });

    test('a reference without the field falls back to the headers', () {
      // Ein Detail aus einem älteren Daemon oder eines, das ein Test von Hand
      // gebaut hat: Ohne diesen Rückfall käme der gepackte Rumpf zurück.
      final BodySource source = BodySource.of(
        refFor(40, contentType: 'application/json'),
        headers: <Header>[
          Header(name: 'Content-Encoding', value: ' GZIP '.codeUnits),
        ],
        findings: const <Finding>[],
      );
      expect(source.encoding, 'gzip');
      expect(source.reference.asking(source.encoding).contentEncoding, 'gzip');
    });

    test('the reference wins over the headers when it carries the field', () {
      final BodySource source = BodySource.of(
        refFor(40, contentEncoding: 'br'),
        headers: <Header>[
          Header(name: 'content-encoding', value: 'gzip'.codeUnits),
        ],
        findings: const <Finding>[],
      );
      expect(source.encoding, 'br');
    });

    test('the header is read case-insensitively', () {
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

  group('the fake answers like the daemon', () {
    test('a brotli body comes back as plain text', () async {
      final FakeDaemonClient client = FakeDaemonClient();
      addTearDown(client.close);
      final BodyRef packed = refFor(0);
      // Den Rumpf des Skripts von Hand ablegen, damit der Test nicht auf die
      // Uhr des Abspielers wartet.
      final Uint8List wire = Uint8List.fromList(<int>[1, 2, 3, 4]);
      final Uint8List plain = bytesOf('{"token": "eyJhbGciOi"}');
      final String key = keyOf(packed);
      client.state.bodies[key] = wire;
      client.state.unpacked[key] = plain;

      final BodyRef reference = packed.copyWith(size: wire.length).asking('br');
      final List<BodyChunk> chunks = await client
          .getBodyChunks(reference)
          .toList();
      expect(chunks.single.encodingLeft, '');
      expect(utf8.decode(chunks.single.data), '{"token": "eyJhbGciOi"}');
    });

    test('gzip is really unpacked, and zstd is not', () async {
      final FakeDaemonClient client = FakeDaemonClient();
      addTearDown(client.close);
      final Uint8List packed = Uint8List.fromList(
        gzip.encode(utf8.encode('{"a":1234567890}')),
      );
      final BodyRef reference = refFor(packed.length);
      client.state.bodies[keyOf(reference)] = packed;

      final List<BodyChunk> unpacked = await client
          .getBodyChunks(reference.asking('gzip'))
          .toList();
      expect(unpacked.single.encodingLeft, '');
      expect(utf8.decode(unpacked.single.data), '{"a":1234567890}');

      final List<BodyChunk> left = await client
          .getBodyChunks(reference.asking('zstd'))
          .toList();
      expect(left.single.encodingLeft, 'zstd');
      expect(left.single.data, packed);
    });

    test('the provider hands the encoding of the chunk to the view', () async {
      // Der Weg, den keine der Funktionen oben abdeckt: vom Stück über
      // `flowBodyProvider` in `RawBody.encodingLeft`. Ohne ihn fragte die
      // Ansicht zwar mit `decoded`, wüsste aber nie, dass etwas liegen blieb,
      // und markierte in gepackten Bytes.
      final FakeDaemonClient client = FakeDaemonClient();
      addTearDown(client.close);
      final Uint8List wire = Uint8List.fromList(<int>[9, 8, 7, 6, 5]);
      final BodyRef reference = refFor(wire.length)
          .copyWith(contentEncoding: 'zstd');
      client.state.bodies[keyOf(reference)] = wire;

      final ProviderContainer container = ProviderContainer(
        overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
      );
      addTearDown(container.dispose);
      final RawBody raw = await container.read(
        flowBodyProvider(reference.asking('zstd')).future,
      );
      expect(raw.encodingLeft, 'zstd');
      expect(raw.bytes, wire);
      expect(raw.short, isFalse, reason: 'the bytes are the recorded ones');
      expect(buildBodyLoad(raw, reference).aligned, isFalse);
    });

    test('a body the daemon unpacked is never called short', () async {
      // `size` zählt die gepackten Bytes. Ein Vergleich gegen die Länge des
      // Entpackten meldete jeden gut komprimierten Rumpf als abgebrochen.
      final FakeDaemonClient client = FakeDaemonClient();
      addTearDown(client.close);
      final String pad = List<String>.filled(512, 'a').join();
      final Uint8List packed = Uint8List.fromList(
        gzip.encode(utf8.encode('{"pad":"$pad"}')),
      );
      final BodyRef reference = refFor(packed.length)
          .copyWith(contentEncoding: 'gzip');
      client.state.bodies[keyOf(reference)] = packed;

      final ProviderContainer container = ProviderContainer(
        overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
      );
      addTearDown(container.dispose);
      final RawBody raw = await container.read(
        flowBodyProvider(reference.asking('gzip')).future,
      );
      expect(raw.bytes.length, greaterThan(reference.size));
      // Auf dem Inhalt bestehen, nicht nur auf der Länge: Rohe Bytes wären
      // ebenfalls länger als nichts, und `short` bliebe auch dann falsch.
      expect(utf8.decode(raw.bytes), '{"pad":"$pad"}');
      expect(raw.encodingLeft, '');
      expect(raw.short, isFalse);
      expect(buildBodyLoad(raw, reference).problem, isNull);
    });

    test('a body that unpacks smaller than it arrived is not short', () async {
      // Der Fall, den der Längenvergleich allein falsch entscheidet: brotli
      // über einem kurzen Rumpf wird größer, nicht kleiner (38 Bytes Klartext
      // wurden zu 42 gepackten). Wer nach dem Auspacken weiter gegen `size`
      // misst, meldet hier einen Abbruch, den es nicht gab.
      final FakeDaemonClient client = FakeDaemonClient();
      addTearDown(client.close);
      final Uint8List wire = Uint8List.fromList(
        List<int>.generate(42, (int i) => (i * 7) % 256),
      );
      final Uint8List plain = bytesOf('{"token":"eyJhbGciOiJIUzI1NiJ9.e30.x"}');
      expect(plain.length, 38);
      expect(wire.length, greaterThan(plain.length));
      final BodyRef reference = refFor(wire.length)
          .copyWith(contentEncoding: 'br');
      client.state.bodies[keyOf(reference)] = wire;
      client.state.unpacked[keyOf(reference)] = plain;

      final ProviderContainer container = ProviderContainer(
        overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
      );
      addTearDown(container.dispose);
      final RawBody raw = await container.read(
        flowBodyProvider(reference.asking('br')).future,
      );
      expect(raw.bytes, plain);
      expect(raw.encodingLeft, '');
      expect(raw.short, isFalse);
      expect(buildBodyLoad(raw, reference).problem, isNull);
    });

    test('a recorded prefix is not called short, a cut stream is', () async {
      // Beide Male kommen weniger Bytes an, als der Verweis nennt, und nur
      // einer der beiden Fälle ist ein Abbruch. Die Unterscheidung trifft
      // `flowBody`, nicht `buildBodyLoad`: Dort steht, was wirklich ankam.
      final FakeDaemonClient client = FakeDaemonClient();
      addTearDown(client.close);
      final Uint8List prefix = bytesOf('0123456789');
      final ProviderContainer container = ProviderContainer(
        overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
      );
      addTearDown(container.dispose);

      final BodyRef recorded = refFor(100, truncated: true);
      client.state.bodies[keyOf(recorded)] = prefix;
      final RawBody kept = await container.read(
        flowBodyProvider(recorded).future,
      );
      expect(kept.bytes, prefix);
      expect(kept.short, isFalse, reason: 'the recording kept only a prefix');
      expect(buildBodyLoad(kept, recorded).problem, isNull);

      final BodyRef whole = refFor(101);
      client.state.bodies[keyOf(whole)] = prefix;
      final RawBody cut = await container.read(flowBodyProvider(whole).future);
      expect(cut.short, isTrue, reason: 'less arrived than the reference says');
      expect(buildBodyLoad(cut, whole).problem, BodyProblem.incomplete);
    });

    test('a packed body cut short on the wire is short too', () async {
      // Der rohe Abruf: `decoded` ist nicht gesetzt, der Daemon schickt
      // `encoding_left` leer, und trotzdem liegt `gzip` noch auf den Bytes.
      // Wer daraus schließt, die Bytes seien entpackt, verschweigt hier den
      // Abbruch.
      final FakeDaemonClient client = FakeDaemonClient();
      addTearDown(client.close);
      final Uint8List half = Uint8List.fromList(<int>[0x1f, 0x8b, 0x08, 0x00]);
      final BodyRef reference = refFor(4096).copyWith(contentEncoding: 'gzip');
      client.state.bodies[keyOf(reference)] = half;

      final ProviderContainer container = ProviderContainer(
        overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
      );
      addTearDown(container.dispose);
      final RawBody raw = await container.read(
        flowBodyProvider(reference).future,
      );
      expect(
        raw.encodingLeft,
        '',
        reason: 'a raw fetch never claims an encoding',
      );
      expect(raw.short, isTrue);
    });

    test('a stream that never ends is short, even unpacked', () async {
      // Ein entpackter Rumpf lässt sich nicht mehr an `size` messen -- das
      // zählt die gepackten Bytes. Die einzige Auskunft, die dann noch gilt,
      // ist das letzte Stück, und dieser Strom schickt keines.
      // `size` absichtlich kleiner als das, was ankommt: Dann kann der
      // Längenvergleich nicht der Grund sein, aus dem der Rumpf als
      // abgebrochen gilt.
      final BodyRef reference = refFor(20)
          .copyWith(contentEncoding: 'br', decoded: true);
      final _EndlessClient client = _EndlessClient();
      addTearDown(client.close);
      final ProviderContainer container = ProviderContainer(
        overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
      );
      addTearDown(container.dispose);
      final RawBody raw = await container.read(
        flowBodyProvider(reference).future,
      );
      expect(raw.bytes, isNotEmpty, reason: 'what arrived stays readable');
      expect(raw.encodingLeft, '');
      expect(
        raw.bytes.length,
        greaterThan(reference.size),
        reason:
            'the length says nothing here; only the missing last chunk does',
      );
      expect(raw.short, isTrue);
    });

    test('without the flag the recorded bytes come back', () async {
      final FakeDaemonClient client = FakeDaemonClient();
      addTearDown(client.close);
      final Uint8List packed = Uint8List.fromList(
        gzip.encode(utf8.encode('{"a":1}')),
      );
      final BodyRef reference = refFor(packed.length)
          .copyWith(contentEncoding: 'gzip');
      client.state.bodies[keyOf(reference)] = packed;
      final List<Uint8List> chunks = await client.getBody(reference).toList();
      expect(chunks.single, packed);
    });
  });

  group('the transport', () {
    test('a short stream is incomplete, never empty', () {
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: bytesOf('0123456789'), short: true),
        refFor(100),
      );
      expect(load.problem, BodyProblem.incomplete);
      expect(load.kind, isNot(BodyKind.empty));
      expect(load.bytes, isNotEmpty);
    });

    test('a body over the cap is too large and keeps what arrived', () {
      final BodyLoad load = buildBodyLoad(
        RawBody(bytes: bytesOf('{"a":1}'), overflowed: true),
        refFor(bodyMaxBytes + 1),
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
      // Derselbe Digest einmal gepackt und einmal entpackt sind zwei
      // Anzeigen. Ohne `decoded` im Schlüssel zeigte die History die Bytes
      // der Warteschlange oder umgekehrt.
      expect(cacheKeyOf(base), isNot(cacheKeyOf(base.copyWith(decoded: true))));
      expect(
        cacheKeyOf(base),
        isNot(cacheKeyOf(base.copyWith(contentEncoding: 'br'))),
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
