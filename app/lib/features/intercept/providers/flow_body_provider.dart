/// Der Rumpf hinter einem [BodyRef]: geholt, gemerkt, und über die Kopfzeilen
/// der Anfrage in den Byteraum gebracht, in dem der Daemon gesucht hat.
///
/// Zwei Provider, weil zwei verschiedene Dinge gemerkt werden. Der kanonische
/// `flowBodyProvider(BodyRef)` hält, was `GetBody` liefert: die Bytes des
/// Transports, unverändert, im Zwischenspeicher. [parsedBodyProvider] hängt
/// zusätzlich am Flow, weil erst dort die Kopfzeilen stehen — und ohne
/// `Content-Encoding` lässt sich weder auspacken noch entscheiden, ob eine
/// Fundstelle überhaupt gezeichnet werden darf (`body_decode.dart`).
///
/// Drei Dinge, die dieser Weg gegen einen feindlichen Rumpf tut:
///
/// * **Er hört auf zu lesen.** Über [bodyMaxBytes] wird der Strom abgebrochen;
///   der Rumpf gilt als [BodyKind.tooLarge], und was schon da ist, bleibt
///   lesbar.
/// * **Er packt begrenzt und nur auf Ansage aus.** Ein gzip-Rumpf von zwei
///   Mebibyte kann sich zu Gigabyte entfalten; das Auspacken läuft gestückelt
///   und bricht an derselben Grenze ab. Ausgepackt wird nur, was der Header
///   nennt.
/// * **Er verschweigt keinen Abbruch.** Bricht der Strom vor der angekündigten
///   Größe ab, endet der gepackte Inhalt nicht an seinem Abschluss oder nennt
///   die Anfrage eine Kodierung, die hier fehlt, kommt das Ergebnis mit einem
///   [BodyProblem] zurück, nie als leerer Rumpf. „Leer" und „nicht lesbar"
///   sind zwei Aussagen, und die zweite darf nie wie die erste aussehen.
library;

import 'dart:async';
import 'dart:collection';
import 'dart:isolate';
import 'dart:typed_data';

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../body/body_decode.dart';
import '../body/body_kind.dart';
import '../body/body_parser.dart';
import 'flows.dart';

part 'flow_body_provider.g.dart';

/// Wie viele Rümpfe der Zwischenspeicher hält.
const int bodyCacheEntries = 32;

/// Wie viele Bytes der Zwischenspeicher hält.
const int bodyCacheBytes = 64 * 1024 * 1024;

/// Der Zwischenspeicher: die letzten [bodyCacheEntries] Rümpfe, höchstens
/// [bodyCacheBytes] zusammen.
///
/// Gemerkt wird über den Digest **und** über alles, was den Verweis sonst noch
/// ausmacht: zwei Anfragen mit demselben Inhalt sind derselbe Rumpf, aber
/// derselbe Digest mit anderem `Content-Type` oder anderer Kürzung ist eine
/// andere Anzeige.
class BodyCache {
  final LinkedHashMap<String, RawBody> _entries =
      LinkedHashMap<String, RawBody>();
  int _bytes = 0;

  /// Wie viele Rümpfe gerade gemerkt sind.
  int get length => _entries.length;

  /// Wie viele Bytes gerade gemerkt sind.
  int get bytes => _bytes;

  /// Der Rumpf zu [key], oder null. Ein Treffer wird der jüngste Eintrag.
  RawBody? read(String key) {
    final RawBody? load = _entries.remove(key);
    if (load != null) {
      _entries[key] = load;
    }
    return load;
  }

  /// Merkt [load] unter [key] und wirft heraus, was über die Grenzen geht.
  void write(String key, RawBody load) {
    final RawBody? old = _entries.remove(key);
    if (old != null) {
      _bytes -= old.weight;
    }
    _entries[key] = load;
    _bytes += load.weight;
    while (_entries.length > bodyCacheEntries ||
        (_bytes > bodyCacheBytes && _entries.length > 1)) {
      final String oldest = _entries.keys.first;
      _bytes -= _entries.remove(oldest)!.weight;
    }
  }
}

/// Der Zwischenspeicher der Sitzung.
@Riverpod(keepAlive: true)
BodyCache bodyCache(Ref ref) => BodyCache();

/// Die Bytes hinter [reference], so wie der Transport sie liefert.
@riverpod
Future<RawBody> flowBody(Ref ref, BodyRef reference) async {
  if (reference.isEmpty) {
    return RawBody.none;
  }
  final BodyCache cache = ref.watch(bodyCacheProvider);
  final String key = cacheKeyOf(reference);
  final RawBody? cached = cache.read(key);
  if (cached != null) {
    return cached;
  }
  final DaemonClient client = ref.watch(daemonClientProvider);
  final BytesBuilder buffer = BytesBuilder(copy: false);
  bool overflowed = false;
  await for (final Uint8List chunk in client.getBody(reference)) {
    buffer.add(chunk);
    if (buffer.length > bodyMaxBytes) {
      overflowed = true;
      break;
    }
  }
  final Uint8List received = buffer.takeBytes();
  final RawBody raw = RawBody(
    bytes: received,
    overflowed: overflowed,
    short: !reference.truncated && received.length < reference.size,
  );
  cache.write(key, raw);
  return raw;
}

/// Der Rumpf zu [flowId], ausgepackt, zerlegt und mit seinen Funden.
@riverpod
Future<ParsedBody> parsedBody(Ref ref, FlowId flowId, BodyRef reference) async {
  final RawBody raw = await ref.watch(flowBodyProvider(reference).future);
  final FlowDetail detail = await ref.watch(flowDetailProvider(flowId).future);
  final String encoding = contentEncodingOf(
    detail.request?.headers ?? const <Header>[],
  );
  final List<Finding> findings = detail.findings;
  if (raw.bytes.length <= bodyIsolateThreshold) {
    return decodeAndParseBody(raw, reference, encoding, findings);
  }
  // Auspacken und Zerlegen zusammen auf das andere Isolat: das Auspacken von
  // acht Mebibyte kostet hier genauso viel wie das Zerlegen (`docs/UX.md` 7).
  return Isolate.run(
    () => decodeAndParseBody(raw, reference, encoding, findings),
  );
}

/// Der Schlüssel, unter dem [reference] gemerkt wird.
String cacheKeyOf(BodyRef reference) {
  final StringBuffer buffer = StringBuffer();
  for (final int byte in reference.sha256) {
    buffer.write(byte.toRadixString(16).padLeft(2, '0'));
  }
  return '$buffer:${reference.size}:${reference.truncated}'
      ':${reference.contentType}';
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
