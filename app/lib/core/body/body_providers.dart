/// Ein Rumpf auf dem Weg in die Ansicht: ausgepackt, zerlegt, mit seinen
/// Funden, und welche der vier Ansichten er gerade zeigt.
///
/// Die Bytes kommen aus `flowBodyProvider`, dem einen Rumpf-Provider der App.
/// Was diese Bytes sind, wissen sie selbst nicht: welche Kodierung auf dem
/// Aufgezeichneten lag, steht im Verweis (und, wo der es nicht trägt, in den
/// Kopfzeilen der Seite), und welche Funde darin liegen, im Detail des Flows.
/// Beides reicht der Aufrufer als [BodySource] herein. Warteschlange und
/// History lesen ihr Detail aus je eigenem Provider, und keines der beiden
/// Features darf den des anderen kennen (`docs/ARCHITECTURE.md` 5).
///
/// Zwei Dinge, die dieser Weg gegen einen feindlichen Rumpf tut:
///
/// * **Er packt nicht selbst aus.** Das tut der Daemon, mit dem Budget des
///   Scans (`limits.preview_cap_bytes`, `limits.max_decompress_ratio`); eine
///   Bombe entfaltet sich damit nie im Prozess der Oberfläche. Was ankommt,
///   begrenzt zusätzlich [bodyMaxBytes] (`flow_body_provider.dart`).
/// * **Er verschweigt keinen Abbruch.** Bleibt eine Kodierung auf den Bytes
///   liegen, weil der Daemon sie nicht kennt, kommt das Ergebnis mit einem
///   [BodyProblem] zurück und ohne Markierungen, nie als leerer Rumpf.
library;

import 'dart:isolate';

import 'package:flutter/foundation.dart' show immutable, listEquals;
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../domain/domain.dart';
import 'flow_body_provider.dart';
import 'body_decode.dart';
import 'body_kind.dart';
import 'body_parser.dart';

part 'body_providers.g.dart';

/// Woher ein Rumpf kommt: der Verweis und das, was der Flow über ihn weiß.
///
/// Der Schlüssel von [parsedBodyProvider]. Er vergleicht die Funde nach
/// Inhalt, nicht nach Identität: ein neu geladenes Detail mit denselben
/// Funden ist derselbe Rumpf und wird nicht noch einmal zerlegt.
@immutable
class BodySource {
  /// Creates a source.
  const BodySource({
    required this.reference,
    required this.encoding,
    required this.findings,
  });

  /// Die Quelle für [reference], mit dem `Content-Encoding` aus [headers].
  ///
  /// [headers] sind die Kopfzeilen der Seite, zu der der Rumpf gehört: die der
  /// Anfrage für den Anfrage-Rumpf, die der Antwort für den Antwort-Rumpf. Sie
  /// gelten nur noch, wo [reference] die Kodierung nicht selbst trägt — ein
  /// Detail aus einem älteren Daemon oder eines, das ein Test von Hand baut.
  /// Der Daemon füllt `BodyRef.content_encoding` aus genau diesen Kopfzeilen
  /// (HUM-119).
  BodySource.of(
    this.reference, {
    required List<Header> headers,
    required this.findings,
  }) : encoding = reference.contentEncoding.isEmpty
           ? contentEncodingOf(headers)
           : reference.contentEncoding;

  /// Der Verweis auf die Bytes.
  final BodyRef reference;

  /// Der `Content-Encoding` der Seite, kleingeschrieben; leer für keinen.
  final String encoding;

  /// Der Verweis, mit dem gefragt wird: entpackt, mit der Kodierung der Seite.
  ///
  /// Der eine Verweis dieser Quelle. `parsedBodyProvider` und jede Ansicht,
  /// die dieselben Bytes noch einmal braucht, gehen über ihn; ein zweiter,
  /// roher Verweis daneben wäre ein zweiter Schlüssel im Zwischenspeicher,
  /// also ein zweites `GetBody` über die Leitung und in der Hex-Ansicht die
  /// gepackten Bytes (HUM-119).
  BodyRef get asked => reference.asking(encoding);

  /// Die Funde, deren Stellen in diesen Bytes liegen.
  ///
  /// Der Daemon sucht auf Anfragen; eine Antwort bekommt deshalb eine leere
  /// Liste, und ebenso ein vom Menschen bearbeiteter Rumpf, weil die
  /// Fundstellen im Original liegen und nicht in der Bearbeitung.
  final List<Finding> findings;

  @override
  bool operator ==(Object other) =>
      other is BodySource &&
      other.reference == reference &&
      other.encoding == encoding &&
      listEquals(other.findings, findings);

  @override
  int get hashCode =>
      Object.hash(reference, encoding, Object.hashAll(findings));
}

/// Der Rumpf aus [source], ausgepackt, zerlegt und mit seinen Funden.
///
/// Gefragt wird immer mit `decoded`: Die Bereiche der Funde zeigen in die
/// entpackten Bytes, also muss die Ansicht die entpackten halten. Was der
/// Daemon nicht abnehmen konnte, sagt er im Stück, und dann wird nichts
/// markiert (HUM-119).
@riverpod
Future<ParsedBody> parsedBody(Ref ref, BodySource source) async {
  final BodyRef reference = source.asked;
  final RawBody raw = await ref.watch(flowBodyProvider(reference).future);
  final List<Finding> findings = source.findings;
  if (raw.bytes.length <= bodyIsolateThreshold) {
    return decodeAndParseBody(raw, reference, findings);
  }
  // Das Zerlegen von acht Mebibyte gehört nicht auf das Isolat der
  // Oberfläche (`docs/UX.md` 7).
  return Isolate.run(() => decodeAndParseBody(raw, reference, findings));
}

/// Die zuletzt gewählte Ansicht der Sitzung, oder null.
///
/// Ein einziger Wert, kein Eintrag je Flow: eine Karte, die für jede jemals
/// gesehene `FlowId` eine Wahl behält, wächst mit dem Verkehr der ganzen
/// Sitzung, und beschränkter Zustand ist eine Regel dieses Programms
/// (`docs/UX.md` 7).
@Riverpod(keepAlive: true)
class LastBodyPane extends _$LastBodyPane {
  @override
  BodyPane? build() => null;

  /// Merkt sich [pane] für den nächsten Flow.
  void remember(BodyPane pane) => state = pane;
}

/// Welche Ansicht ein Flow zeigt.
///
/// `null` heißt „die Vorauswahl seiner Art". Solange die Karte steht, gehört
/// die Wahl diesem Flow; danach erbt sie der nächste über [LastBodyPane]. Ein
/// Mensch, der auf Hex gestellt hat, will das beim nächsten `J` nicht wieder
/// tun — und das Programm will dafür keine Karte, die nie aufhört zu wachsen.
@riverpod
class BodyViewMode extends _$BodyViewMode {
  @override
  BodyPane? build(FlowId flowId) => ref.watch(lastBodyPaneProvider);

  /// Stellt auf [pane].
  void select(BodyPane pane) {
    state = pane;
    ref.read(lastBodyPaneProvider.notifier).remember(pane);
  }
}
