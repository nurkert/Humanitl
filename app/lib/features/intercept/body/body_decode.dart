/// Vom Transport zum Byteraum, in dem der Daemon gesucht hat.
///
/// `GetBody` liefert die Bytes so, wie der Recorder sie gespeichert hat: roh,
/// also gepackt, wenn die Anfrage gepackt war. Der Daemon dagegen entpackt vor
/// dem Suchen und meldet seine Fundstellen auf den **entpackten** Bytes
/// (`daemon/crates/findings/src/decode.rs`). Beide Seiten müssen deshalb im
/// selben Byteraum landen, sonst zeigt eine Markierung auf einen Wert, der
/// dort nie stand.
///
/// Daraus folgt alles hier:
///
/// * **Der Header entscheidet, nicht die ersten zwei Bytes.** Ein Upload ohne
///   `Content-Encoding`, der zufällig mit `1F 8B` beginnt, wird nicht
///   ausgepackt — der Daemon hat ihn roh durchsucht. Umgekehrt wird ein
///   deklariertes `gzip` ausgepackt, auch wenn es nicht danach aussieht.
/// * **Was diese Ansicht nicht auspacken kann, bekommt keine Fundstelle.**
///   `br` und `zstd` stehen im Vertrag, aber nicht in `dart:io`. Dann werden
///   die Rohbytes gezeigt, jeder Fund behält seinen Namen, und keiner bekommt
///   eine Stelle.
/// * **Ein Strom, der nicht dort endet, wo er es sagt, gilt als abgeschnitten.**
///   Der Dekodierer von Dart wirft dabei nicht; er liefert die Teilausgabe und
///   meldet Erfolg. Geprüft wird deshalb der Abschluss selbst: bei gzip die
///   `ISIZE` der letzten vier Bytes, bei zlib die Adler-32-Summe. Passt sie
///   nicht, ist der Inhalt unvollständig oder mehrgliedrig, und in beiden
///   Fällen hat der Daemon etwas anderes gesehen als wir.
///
/// Frei von Flutter: der ganze Weg läuft über [bodyIsolateThreshold] in
/// `Isolate.run` (`docs/UX.md` 7).
library;

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import '../../../core/domain/domain.dart';
import 'body_kind.dart';
import 'body_parser.dart';

/// Die Kodierungen, die dieser Weg kennt.
enum BodyEncoding {
  /// Kein `Content-Encoding`, oder `identity`.
  identity,

  /// `gzip` oder `x-gzip`.
  gzip,

  /// `deflate`, als zlib-Strom.
  zlib,

  /// Etwas, das diese Ansicht nicht auspacken kann: `br`, `zstd`, eine Kette.
  unsupported,
}

/// Der Wert des `Content-Encoding` in [headers], kleingeschrieben.
String contentEncodingOf(List<Header> headers) {
  for (final Header header in headers) {
    if (header.name.toLowerCase() == 'content-encoding') {
      return header.text.trim().toLowerCase();
    }
  }
  return '';
}

/// Wie [value] zu behandeln ist.
///
/// Eine Kette wie `gzip, br` gilt als nicht auspackbar: die zweite Schicht
/// fehlt hier, und halb ausgepackt ist kein Byteraum.
BodyEncoding encodingOf(String value) => switch (value) {
  '' || 'identity' => BodyEncoding.identity,
  'gzip' || 'x-gzip' => BodyEncoding.gzip,
  'deflate' => BodyEncoding.zlib,
  _ => BodyEncoding.unsupported,
};

/// Die Bytes, wie sie über `GetBody` ankamen.
class RawBody {
  /// Creates a transport result.
  const RawBody({
    required this.bytes,
    this.overflowed = false,
    this.short = false,
  });

  /// Nichts angekommen.
  static final RawBody none = RawBody(bytes: Uint8List(0));

  /// Die Bytes, höchstens [bodyMaxBytes] plus ein Stück.
  final Uint8List bytes;

  /// Wahr, wenn der Strom an der Obergrenze abgebrochen wurde.
  final bool overflowed;

  /// Wahr, wenn weniger ankam, als der Verweis nennt.
  final bool short;

  /// Wie viel Platz dieser Eintrag im Zwischenspeicher belegt.
  int get weight => bytes.lengthInBytes;
}

/// Das Ergebnis des Auspackens.
class InflatedBody {
  /// Creates a result.
  const InflatedBody(
    this.bytes, {
    this.decompressed = false,
    this.aborted = false,
    this.overflowed = false,
    this.undecoded = false,
    this.aligned = true,
  });

  /// Die Bytes, ausgepackt oder unverändert.
  final Uint8List bytes;

  /// Wahr, wenn ausgepackt wurde.
  final bool decompressed;

  /// Wahr, wenn der Strom vorzeitig endet oder sein Abschluss nicht stimmt.
  final bool aborted;

  /// Wahr, wenn die Grenze gerissen wurde.
  final bool overflowed;

  /// Wahr, wenn eine angekündigte Kodierung nicht ausgepackt wurde.
  final bool undecoded;

  /// Wahr, wenn diese Bytes die sind, auf denen der Daemon gesucht hat.
  final bool aligned;
}

/// Ein geladener Rumpf, so wie er in die Ansicht geht.
class BodyLoad {
  /// Creates a load.
  const BodyLoad({
    required this.bytes,
    required this.kind,
    required this.declaredSize,
    required this.contentType,
    required this.encoding,
    this.disputedType = false,
    this.decompressed = false,
    this.aligned = true,
    this.problem,
  });

  /// Ein Rumpf, den niemand geschickt hat.
  static final BodyLoad none = BodyLoad(
    bytes: Uint8List(0),
    kind: BodyKind.empty,
    declaredSize: 0,
    contentType: '',
    encoding: '',
  );

  /// Die Bytes, die gezeigt werden.
  final Uint8List bytes;

  /// Wie der Rumpf angezeigt wird.
  final BodyKind kind;

  /// Die Größe, die der Verweis nennt; sie zählt die **gepackten** Bytes.
  final int declaredSize;

  /// Der `Content-Type` des Verweises.
  final String contentType;

  /// Der `Content-Encoding` der Anfrage, kleingeschrieben.
  final String encoding;

  /// Wahr, wenn der `Content-Type` etwas anderes sagt als die Bytes zeigen.
  final bool disputedType;

  /// Wahr, wenn der Rumpf ausgepackt wurde.
  final bool decompressed;

  /// Wahr, wenn diese Bytes die sind, auf denen der Daemon gesucht hat.
  final bool aligned;

  /// Warum weniger da ist, als der Verweis ankündigt, oder null.
  final BodyProblem? problem;
}

/// Was aus [raw] und [reference] wird, wenn die Anfrage [encoding] ankündigt.
BodyLoad buildBodyLoad(RawBody raw, BodyRef reference, String encoding) {
  final BodyEncoding declared = encodingOf(encoding);
  final InflatedBody inflated = inflateBody(raw.bytes, declared);
  final Uint8List bytes = inflated.bytes;
  BodyProblem? problem;
  if (raw.overflowed || inflated.overflowed) {
    problem = BodyProblem.tooLarge;
  } else if (inflated.undecoded) {
    problem = BodyProblem.undecodedEncoding;
  } else if (inflated.aborted) {
    problem = BodyProblem.truncatedStream;
  } else if (!reference.truncated && raw.bytes.length < reference.size) {
    problem = BodyProblem.incomplete;
  }
  final int size = raw.overflowed || inflated.overflowed
      ? bodyMaxBytes + 1
      : bytes.length;
  return BodyLoad(
    bytes: bytes,
    kind: detectBodyKind(bytes, reference.contentType, totalSize: size),
    declaredSize: reference.size,
    contentType: reference.contentType,
    encoding: encoding,
    // Die Streitfrage stellt sich nur bei ungepackten Bytes. Ein deklariertes
    // gzip, das nicht aufgeht, ist keine falsche Typangabe, und der Satz
    // dazu wäre eine falsche Erklärung für ein echtes Problem.
    disputedType:
        declared == BodyEncoding.identity &&
        !inflated.undecoded &&
        bodyTypeIsDisputed(bytes, reference.contentType),
    decompressed: inflated.decompressed,
    aligned: inflated.aligned,
    problem: problem,
  );
}

/// Holt, packt aus und zerlegt in einem Zug.
///
/// Eine Funktion, damit der ganze Weg in `Isolate.run` passt: das Auspacken
/// von acht Mebibyte kostet auf dem UI-Isolat genauso viel wie das Zerlegen.
ParsedBody decodeAndParseBody(
  RawBody raw,
  BodyRef reference,
  String encoding,
  List<Finding> findings,
) {
  final BodyLoad load = buildBodyLoad(raw, reference, encoding);
  return parseLoadedBody(load, findings);
}

/// Zerlegt, was [load] trägt.
ParsedBody parseLoadedBody(BodyLoad load, List<Finding> findings) {
  if (load.kind == BodyKind.tooLarge) {
    // Nichts wird zerlegt; die Ansicht zeigt Größe, Typ und den Anfang.
    return parseBody(
      Uint8List.sublistView(
        load.bytes,
        0,
        load.bytes.length < bodyHexLimit ? load.bytes.length : bodyHexLimit,
      ),
      BodyKind.text,
      findings,
      problem: BodyProblem.tooLarge,
      disputedType: load.disputedType,
      placeFindings: load.aligned,
      encodingLabel: load.encoding,
    );
  }
  return parseBody(
    load.bytes,
    load.kind,
    findings,
    disputedType: load.disputedType,
    problem: load.problem,
    placeFindings: load.aligned,
    encodingLabel: load.encoding,
  );
}

/// Packt [bytes] aus, wenn [encoding] es verlangt.
InflatedBody inflateBody(Uint8List bytes, BodyEncoding encoding) {
  switch (encoding) {
    case BodyEncoding.identity:
      return InflatedBody(bytes);
    case BodyEncoding.unsupported:
      // Der Daemon hat den entpackten Inhalt durchsucht, diese Ansicht hat
      // ihn nicht. Die Rohbytes sind ehrlicher als ein geratener Inhalt, und
      // ohne gemeinsamen Byteraum wird nichts markiert.
      return InflatedBody(bytes, undecoded: true, aligned: false);
    case BodyEncoding.gzip:
    case BodyEncoding.zlib:
      break;
  }
  final BytesBuilder out = BytesBuilder(copy: false);
  bool overflowed = false;
  final _ChunkSink sink = _ChunkSink((List<int> chunk) {
    out.add(chunk);
    // Erst anhängen, dann prüfen: ein Strom, dessen letztes Stück die Grenze
    // reißt, muss auch abbrechen, und nicht erst das Stück danach.
    if (out.length > bodyMaxBytes) {
      overflowed = true;
      throw const _BombException();
    }
  });
  try {
    final ByteConversionSink inflater =
        (encoding == BodyEncoding.gzip ? gzip.decoder : zlib.decoder)
            .startChunkedConversion(sink);
    inflater
      ..add(bytes)
      ..close();
  } on _BombException {
    return InflatedBody(
      out.takeBytes(),
      decompressed: true,
      overflowed: overflowed,
      aborted: true,
      aligned: false,
    );
  } on Object {
    // Kein gültiger Strom. Was schon ausgepackt ist, ist weniger wert als die
    // Wahrheit darüber, also bleiben die Rohbytes stehen und der Fall bekommt
    // seinen eigenen Namen.
    return InflatedBody(bytes, undecoded: true, aborted: true, aligned: false);
  }
  final Uint8List result = out.takeBytes();
  if (result.isEmpty && bytes.isNotEmpty) {
    return InflatedBody(bytes, undecoded: true, aborted: true, aligned: false);
  }
  final bool complete = encoding == BodyEncoding.gzip
      ? _gzipTrailerFits(bytes, result)
      : _zlibTrailerFits(bytes, result);
  return InflatedBody(
    result,
    decompressed: true,
    aborted: !complete,
    aligned: complete,
  );
}

/// Wahr, wenn die `ISIZE` am Ende von [packed] zur Länge von [out] passt.
///
/// Der Dekodierer meldet bei einem abgeschnittenen Strom Erfolg mit
/// Teilausgabe; erst diese Zahl verrät den Abbruch. Sie schlägt auch bei einem
/// mehrgliedrigen gzip an — dort steht die Länge des letzten Glieds —, und das
/// ist gewollt: `flate2` im Daemon liest heute nur das erste Glied, also
/// stimmen die Byteräume dann ohnehin nicht überein.
bool _gzipTrailerFits(Uint8List packed, Uint8List out) {
  if (packed.length < 8) {
    return false;
  }
  final int isize =
      packed[packed.length - 4] |
      (packed[packed.length - 3] << 8) |
      (packed[packed.length - 2] << 16) |
      (packed[packed.length - 1] << 24);
  return (isize & 0xFFFFFFFF) == (out.length & 0xFFFFFFFF);
}

/// Wahr, wenn die Adler-32-Summe am Ende von [packed] zu [out] passt.
bool _zlibTrailerFits(Uint8List packed, Uint8List out) {
  if (packed.length < 6) {
    return false;
  }
  final int stored =
      (packed[packed.length - 4] << 24) |
      (packed[packed.length - 3] << 16) |
      (packed[packed.length - 2] << 8) |
      packed[packed.length - 1];
  return (stored & 0xFFFFFFFF) == adler32(out);
}

/// Die Adler-32-Summe von [bytes].
int adler32(Uint8List bytes) {
  int a = 1;
  int b = 0;
  for (final int byte in bytes) {
    a = (a + byte) % 65521;
    b = (b + a) % 65521;
  }
  return ((b << 16) | a) & 0xFFFFFFFF;
}

/// Reicht die ausgepackten Stücke weiter.
class _ChunkSink implements Sink<List<int>> {
  _ChunkSink(this._onChunk);

  final void Function(List<int> chunk) _onChunk;

  @override
  void add(List<int> data) => _onChunk(data);

  @override
  void close() {}
}

/// Das Auspacken hat die Grenze gerissen.
class _BombException implements Exception {
  const _BombException();
}
