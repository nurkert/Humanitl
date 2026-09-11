/// Ein Rumpf auf dem Weg in die Ansicht: ausgepackt, zerlegt, mit seinen
/// Funden, und welche der vier Ansichten er gerade zeigt.
///
/// Die Bytes kommen aus `flowBodyProvider`, dem einen Rumpf-Provider in
/// `core/ipc`. Was diese Bytes sind, wissen sie selbst nicht: ob sie gepackt
/// sind, steht in den Kopfzeilen der Seite, zu der der Rumpf gehört, und
/// welche Funde in ihnen liegen, im Detail des Flows. Beides reicht der
/// Aufrufer als [BodySource] herein. Warteschlange und History lesen ihr
/// Detail aus je eigenem Provider, und keines der beiden Features darf den
/// des anderen kennen (`docs/ARCHITECTURE.md` 5).
///
/// Zwei Dinge, die dieser Weg gegen einen feindlichen Rumpf tut:
///
/// * **Er packt begrenzt und nur auf Ansage aus.** Ein gzip-Rumpf von zwei
///   Mebibyte kann sich zu Gigabyte entfalten; das Auspacken läuft gestückelt
///   und bricht an [bodyMaxBytes] ab. Ausgepackt wird nur, was der Header
///   nennt (`body_decode.dart`).
/// * **Er verschweigt keinen Abbruch.** Endet der gepackte Inhalt nicht an
///   seinem Abschluss oder nennt die Seite eine Kodierung, die hier fehlt,
///   kommt das Ergebnis mit einem [BodyProblem] zurück, nie als leerer Rumpf.
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
  /// Anfrage für den Anfrage-Rumpf, die der Antwort für den Antwort-Rumpf.
  /// Wer die falschen reicht, packt einen gzip-Rumpf nicht aus oder einen
  /// ungepackten ein zweites Mal.
  BodySource.of(
    this.reference, {
    required List<Header> headers,
    required this.findings,
  }) : encoding = contentEncodingOf(headers);

  /// Der Verweis auf die Bytes.
  final BodyRef reference;

  /// Der `Content-Encoding` der Seite, kleingeschrieben; leer für keinen.
  final String encoding;

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
@riverpod
Future<ParsedBody> parsedBody(Ref ref, BodySource source) async {
  final BodyRef reference = source.reference;
  final RawBody raw = await ref.watch(flowBodyProvider(reference).future);
  final String encoding = source.encoding;
  final List<Finding> findings = source.findings;
  if (raw.bytes.length <= bodyIsolateThreshold) {
    return decodeAndParseBody(raw, reference, encoding, findings);
  }
  // Auspacken und Zerlegen zusammen auf das andere Isolat: das Auspacken von
  // acht Mebibyte kostet hier genauso viel wie das Zerlegen (`docs/UX.md` 7).
  return Isolate.run(
    () => decodeAndParseBody(raw, reference, encoding, findings),
  );
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
