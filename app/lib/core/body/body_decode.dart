/// Vom Transport zum Byteraum, in dem der Daemon gesucht hat.
///
/// Der Daemon entpackt vor dem Suchen und meldet seine Fundstellen auf den
/// **entpackten** Bytes (`daemon/crates/findings/src/decode.rs`). Damit eine
/// Markierung auf denselben Wert zeigt, muss diese Ansicht dieselben Bytes
/// halten. Seit HUM-119 bittet sie deshalb `GetBody` darum, die Kodierung
/// abzunehmen (`BodyRef.decoded`), statt selbst auszupacken:
///
/// * **Der Daemon kann mehr.** `br` gehört zum Vertrag und fehlt in `dart:io`.
///   Vorher behielt jeder Fund in einem brotli-Rumpf seinen Namen und verlor
///   seine Stelle; jetzt kommt der Klartext an, und der Bereich trifft.
/// * **Ein Entpacker weniger.** Bomben, abgeschnittene Ströme und mehrgliedrige
///   Archive prüft der Daemon, mit dem Budget aus `limits.preview_cap_bytes`
///   und `limits.max_decompress_ratio`. Ein zweiter Entpacker hier hieße ein
///   zweites Urteil über dieselben Bytes, und zwei Urteile widersprechen sich
///   irgendwann.
/// * **Was übrig bleibt, steht im Stück.** `BodyChunk.encoding_left` ist leer,
///   wenn die Bytes der Byteraum der Funde sind, und nennt sonst die Kodierung,
///   die der Daemon nicht abnehmen konnte (`zstd` oder eine Kette aus zwei
///   Schichten). Dann werden die Rohbytes gezeigt, jeder Fund behält seinen
///   Namen, und keiner bekommt eine Stelle.
///
/// Frei von Flutter: der ganze Weg läuft über [bodyIsolateThreshold] in
/// `Isolate.run` (`docs/UX.md` 7).
library;

import 'dart:typed_data';

import '../domain/domain.dart';
import 'body_kind.dart';
import 'body_parser.dart';

/// Der Wert des `Content-Encoding` in [headers], kleingeschrieben.
///
/// Die Quelle, aus der auch der Daemon `BodyRef.content_encoding` füllt. Sie
/// wird noch gebraucht, wo ein Verweis das Feld nicht trägt: ein Detail, das
/// ein Test von Hand baut, oder ein Daemon mit einer älteren Nebenversion.
String contentEncodingOf(List<Header> headers) {
  for (final Header header in headers) {
    if (header.name.toLowerCase() == 'content-encoding') {
      return header.text.trim().toLowerCase();
    }
  }
  return '';
}

/// Die Bytes, wie sie über `GetBody` ankamen.
class RawBody {
  /// Creates a transport result.
  const RawBody({
    required this.bytes,
    this.encodingLeft = '',
    this.overflowed = false,
    this.short = false,
  });

  /// Nichts angekommen.
  static final RawBody none = RawBody(bytes: Uint8List(0));

  /// Die Bytes, höchstens [bodyMaxBytes] plus ein Stück.
  final Uint8List bytes;

  /// Die Kodierung, die noch auf [bytes] liegt; leer für keine.
  ///
  /// Leer heißt: Diese Bytes sind die, auf denen der Daemon gesucht hat.
  /// Steht ein Name darin, konnte der Daemon die Kodierung nicht abnehmen, und
  /// die Bytes sind die der Leitung.
  final String encodingLeft;

  /// Wahr, wenn der Strom an der Obergrenze abgebrochen wurde.
  final bool overflowed;

  /// Wahr, wenn weniger ankam, als der Verweis nennt.
  final bool short;

  /// Wie viel Platz dieser Eintrag im Zwischenspeicher belegt.
  int get weight => bytes.lengthInBytes;
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

  /// Die Kodierung, um die es geht, kleingeschrieben.
  ///
  /// Die, die noch auf den Bytes liegt, wenn der Daemon sie nicht abnehmen
  /// konnte; sonst die, die er abgenommen hat. Leer, wenn es keine gab.
  final String encoding;

  /// Wahr, wenn der `Content-Type` etwas anderes sagt als die Bytes zeigen.
  final bool disputedType;

  /// Wahr, wenn der Daemon den Rumpf ausgepackt hat.
  final bool decompressed;

  /// Wahr, wenn diese Bytes die sind, auf denen der Daemon gesucht hat.
  final bool aligned;

  /// Warum weniger da ist, als der Verweis ankündigt, oder null.
  final BodyProblem? problem;
}

/// Was aus [raw] und [reference] wird.
///
/// Die Kodierung kommt nicht mehr aus den Kopfzeilen der Seite, sondern aus
/// der Antwort selbst: [RawBody.encodingLeft] sagt, was der Daemon liegen
/// lassen musste, und [BodyRef.contentEncoding], worum es überhaupt ging.
BodyLoad buildBodyLoad(RawBody raw, BodyRef reference) {
  final String left = raw.encodingLeft;
  final bool aligned = left.isEmpty;
  final Uint8List bytes = raw.bytes;
  BodyProblem? problem;
  if (raw.overflowed) {
    problem = BodyProblem.tooLarge;
  } else if (!aligned) {
    problem = BodyProblem.undecodedEncoding;
  } else if (raw.short) {
    problem = BodyProblem.incomplete;
  }
  final int size = raw.overflowed ? bodyMaxBytes + 1 : bytes.length;
  return BodyLoad(
    bytes: bytes,
    kind: detectBodyKind(bytes, reference.contentType, totalSize: size),
    declaredSize: reference.size,
    contentType: reference.contentType,
    encoding: left.isEmpty ? reference.contentEncoding : left,
    // Die Streitfrage stellt sich nur, wenn die Bytes das sind, was sie zu
    // sein behaupten. Auf einem Rumpf, der noch gepackt ist, wäre der Satz
    // „content type says text, bytes are not text" eine falsche Erklärung für
    // ein echtes Problem.
    disputedType: aligned && bodyTypeIsDisputed(bytes, reference.contentType),
    decompressed: aligned && reference.contentEncoding.isNotEmpty,
    aligned: aligned,
    problem: problem,
  );
}

/// Holt, ordnet ein und zerlegt in einem Zug.
///
/// Eine Funktion, damit der ganze Weg in `Isolate.run` passt: das Zerlegen von
/// acht Mebibyte gehört nicht auf das Isolat der Oberfläche.
ParsedBody decodeAndParseBody(
  RawBody raw,
  BodyRef reference,
  List<Finding> findings,
) => parseLoadedBody(buildBodyLoad(raw, reference), findings);

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
